"""Inspect FastAPI source using Python's AST and emit an assessment as JSON."""

import argparse
import ast
import json
import os
import re
import sys
from pathlib import Path

HTTP_METHODS = {"get", "post", "put", "patch", "delete", "options", "head", "trace"}
IGNORED_DIRECTORIES = {
    ".git",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
}
MAX_SOURCE_BYTES = 1024 * 1024
MAX_SOURCE_FILES = 10000
RUST_KEYWORDS = {
    "as",
    "break",
    "const",
    "continue",
    "crate",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "async",
    "await",
    "dyn",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "gen",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
}


def diagnostic(path, line, code, message):
    return {"file": path, "line": line, "code": code, "message": message}


def qualified_name(node):
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        base = qualified_name(node.value)
        return base + "." + node.attr if base else None
    return None


def static_string(node):
    return (
        node.value
        if isinstance(node, ast.Constant) and isinstance(node.value, str)
        else None
    )


def keyword(call, name):
    return next((item.value for item in call.keywords if item.arg == name), None)


def named_calls(node, names):
    found = set()
    if node is not None:
        for child in ast.walk(node):
            if isinstance(child, ast.Call):
                name = qualified_name(child.func)
                if name and name.rsplit(".", 1)[-1] in names:
                    found.add(name.rsplit(".", 1)[-1])
    return sorted(found)


def python_type(node):
    name = qualified_name(node) if node is not None else None
    if name in {"str", "bytes", "bool"}:
        return {
            "python": name,
            "candidate": {"str": "string", "bytes": "bytes", "bool": "bool"}[name],
            "review": "Confirm HTTP validation and serialization semantics.",
        }
    if name == "int":
        return {
            "python": name,
            "candidate": None,
            "review": "Python integers are signed and unbounded; confirm range before choosing u32 or u64.",
        }
    if node is None:
        return {
            "python": None,
            "candidate": None,
            "review": "Type annotation is missing.",
        }
    return {
        "python": ast.unparse(node),
        "candidate": None,
        "review": "Type needs manual mapping or is outside the initial contract subset.",
    }


def valid_field_identifier(name):
    return (
        re.fullmatch(r"[a-z][a-z0-9_]*", name) is not None and name not in RUST_KEYWORDS
    )


def field(name, annotation, required, source):
    typ = python_type(annotation)
    reasons = []
    if typ["candidate"] is None:
        reasons.append(typ["review"])
    if not required:
        reasons.append(
            "Optional or defaulted fields have no automatic mapping in the required-field contract."
        )
    if not valid_field_identifier(name):
        reasons.append(
            "Field name must be lowercase ASCII snake_case and cannot be a Rust keyword."
        )
    return {
        "name": name,
        "source": source,
        "required": required,
        "type": typ,
        "blockers": reasons,
    }


def model_fields(model, models):
    if model not in models:
        return None
    return models[model]


def collect_models(tree, base_names):
    models = {}
    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        if not any(qualified_name(base) in base_names for base in node.bases):
            continue
        entries = []
        for item in node.body:
            if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
                required = item.value is None or (
                    isinstance(item.value, ast.Constant)
                    and item.value.value is Ellipsis
                )
                entries.append(
                    field(item.target.id, item.annotation, required, "model")
                )
        models[node.name] = entries
    return models


def parameter_fields(function, models, path, dependency_names):
    args = function.args.posonlyargs + function.args.args
    defaults = [None] * (len(args) - len(function.args.defaults)) + list(
        function.args.defaults
    )
    args += function.args.kwonlyargs
    defaults += function.args.kw_defaults
    fields = []
    blockers = []
    signals = set()
    if function.args.vararg or function.args.kwarg:
        blockers.append("Variadic parameters cannot be mapped automatically.")
    for arg, default in zip(args, defaults):
        signals.update(named_calls(default, dependency_names))
        signals.update(named_calls(arg.annotation, dependency_names))
        if named_calls(default, dependency_names) or named_calls(
            arg.annotation, dependency_names
        ):
            blockers.append(
                "Dependency injection must be reviewed before mapping parameters."
            )
            continue
        name = qualified_name(arg.annotation)
        known = model_fields(name, models)
        if known is not None:
            fields.extend([dict(item, source="body:" + arg.arg) for item in known])
            if default is not None:
                blockers.append(
                    "Defaulted model parameters require manual contract mapping."
                )
            continue
        source = "path" if path and ("{" + arg.arg + "}") in path else "query_or_body"
        fields.append(field(arg.arg, arg.annotation, default is None, source))
    return fields, blockers, sorted(signals)


def response_fields(route, function, models):
    node = keyword(route, "response_model")
    if node is None:
        node = function.returns
    model_name = qualified_name(node)
    if model_name in models:
        return [dict(item, source="response_model") for item in models[model_name]], []
    if node is None:
        return [], ["Response model is missing or dynamic."]
    return [], ["Response model is not a statically resolved local Pydantic BaseModel."]


def parse_file(path, display):
    diagnostics = []
    try:
        if path.stat().st_size > MAX_SOURCE_BYTES:
            return [], [
                diagnostic(
                    display,
                    None,
                    "file_too_large",
                    "Source exceeds the 1 MiB scan limit.",
                )
            ]
        with path.open("rb") as stream:
            source = stream.read(MAX_SOURCE_BYTES + 1)
        if len(source) > MAX_SOURCE_BYTES:
            return [], [
                diagnostic(
                    display,
                    None,
                    "file_too_large",
                    "Source exceeds the 1 MiB scan limit.",
                )
            ]
        tree = ast.parse(source, filename=display)
    except (OSError, SyntaxError, UnicodeError, ValueError) as error:
        message = (
            getattr(error, "msg", None)
            or getattr(error, "strerror", None)
            or error.__class__.__name__
        )
        return [], [
            diagnostic(
                display,
                getattr(error, "lineno", None),
                "parse_error",
                "Source could not be read or parsed: " + message,
            )
        ]

    constructors = {"FastAPI": set(), "APIRouter": set()}
    bases = set()
    dependency_names = {"Depends", "Security"}
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and node.level == 0:
            for alias in node.names:
                if node.module == "fastapi" and alias.name in constructors:
                    constructors[alias.name].add(alias.asname or alias.name)
                if node.module == "fastapi" and alias.name in {"Depends", "Security"}:
                    dependency_names.add(alias.asname or alias.name)
                if node.module == "pydantic" and alias.name == "BaseModel":
                    bases.add(alias.asname or alias.name)
        elif isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "fastapi":
                    prefix = alias.asname or "fastapi"
                    constructors["FastAPI"].add(prefix + ".FastAPI")
                    constructors["APIRouter"].add(prefix + ".APIRouter")
                if alias.name == "pydantic":
                    bases.add((alias.asname or "pydantic") + ".BaseModel")
    models = collect_models(tree, bases)
    instances = {}
    ambiguous_instances = set()
    mounts = {}
    for node in tree.body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            value = node.value
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            names = [target.id for target in targets if isinstance(target, ast.Name)]
            for name in names:
                if name in instances:
                    diagnostics.append(
                        diagnostic(
                            display,
                            node.lineno,
                            "reassigned_instance",
                            "A FastAPI instance name is reassigned; registrations may be ambiguous.",
                        )
                    )
                    instances.pop(name)
                    ambiguous_instances.add(name)
            if isinstance(value, ast.Call):
                called = qualified_name(value.func)
                kind = next(
                    (key for key, aliases in constructors.items() if called in aliases),
                    None,
                )
                if kind:
                    prefix_node = keyword(value, "prefix")
                    prefix = "" if prefix_node is None else static_string(prefix_node)
                    if any(name in ambiguous_instances for name in names):
                        prefix = None
                    if any(item.arg is None for item in value.keywords):
                        diagnostics.append(
                            diagnostic(
                                display,
                                node.lineno,
                                "dynamic_construction",
                                "Expanded constructor options require manual review.",
                            )
                        )
                    for name in names:
                        instances[name] = {
                            "kind": kind,
                            "prefix": prefix,
                            "line": node.lineno,
                            "dependencies": keyword(value, "dependencies") is not None,
                        }
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
            call = node.value
            if isinstance(call.func, ast.Attribute) and call.func.attr in {
                "include_router",
                "add_api_route",
                "add_route",
            }:
                owner = qualified_name(call.func.value)
                if (
                    call.func.attr == "include_router"
                    and owner in instances
                    and instances[owner]["kind"] == "FastAPI"
                    and call.args
                    and not any(item.arg is None for item in call.keywords)
                ):
                    router = qualified_name(call.args[0])
                    prefix_node = keyword(call, "prefix")
                    prefix = "" if prefix_node is None else static_string(prefix_node)
                    if (
                        router in instances
                        and instances[router]["kind"] == "APIRouter"
                        and prefix is not None
                    ):
                        mounts.setdefault(router, []).append(
                            {
                                "prefix": prefix,
                                "owner": owner,
                                "line": node.lineno,
                                "dependencies": keyword(call, "dependencies")
                                is not None
                                or instances[owner]["dependencies"],
                            }
                        )
                        continue
                diagnostics.append(
                    diagnostic(
                        display,
                        node.lineno,
                        "dynamic_registration",
                        "Route registration or router mounting requires manual resolution.",
                    )
                )

    endpoints = []
    for node in tree.body:
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        for decorator in node.decorator_list:
            if not isinstance(decorator, ast.Call) or not isinstance(
                decorator.func, ast.Attribute
            ):
                continue
            owner = qualified_name(decorator.func.value)
            method = decorator.func.attr
            if owner not in instances:
                continue
            if method not in HTTP_METHODS:
                if method in {"api_route", "route", "websocket", "websocket_route"}:
                    diagnostics.append(
                        diagnostic(
                            display,
                            decorator.lineno,
                            "unsupported_decorator",
                            "Dynamic, multi-method, or WebSocket route needs manual review.",
                        )
                    )
                continue
            path = (
                static_string(decorator.args[0])
                if decorator.args
                else static_string(keyword(decorator, "path"))
            )
            instance = instances[owner]
            prefix = instance["prefix"]
            local_path = (
                (prefix + path) if prefix is not None and path is not None else None
            )
            if instance["kind"] == "APIRouter":
                attached = [
                    mount
                    for mount in mounts.get(owner, [])
                    if mount["line"] > node.end_lineno
                ]
                if len(attached) != len(mounts.get(owner, [])):
                    diagnostics.append(
                        diagnostic(
                            display,
                            decorator.lineno,
                            "mount_before_route",
                            "Router inclusion occurs before this route is registered.",
                        )
                    )
            else:
                attached = [
                    {
                        "prefix": "",
                        "owner": owner,
                        "line": instance["line"],
                        "dependencies": instance["dependencies"],
                    }
                ]
            if not attached:
                attached = [
                    {"prefix": None, "owner": None, "line": None, "dependencies": False}
                ]
            for mount in attached:
                full_path = (
                    mount["prefix"] + local_path
                    if mount["prefix"] is not None and local_path is not None
                    else None
                )
                request, blockers, signals = parameter_fields(
                    node, models, full_path or path, dependency_names
                )
                response, response_blockers = response_fields(decorator, node, models)
                blockers.extend(response_blockers)
                blockers.append(
                    "Authentication, authorization, middleware, side effects, HTTP semantics, and operation ID require review."
                )
                if not valid_field_identifier(node.name) or node.name == "new":
                    blockers.append(
                        "Method name must be lowercase ASCII snake_case and cannot be a Rust keyword or new."
                    )
                if full_path is None:
                    blockers.append(
                        "Router mount or route path could not be resolved statically."
                    )
                if len(node.decorator_list) > 1:
                    blockers.append(
                        "Additional decorators or registrations require manual review."
                    )
                if any(item.arg is None for item in decorator.keywords):
                    blockers.append("Expanded route options require manual review.")
                blockers.extend(
                    reason for item in request + response for reason in item["blockers"]
                )
                for label, fields in (("request", request), ("response", response)):
                    names = [item["name"] for item in fields]
                    if len(names) != len(set(names)):
                        blockers.append(
                            "Duplicate "
                            + label
                            + " field names require manual mapping."
                        )
                signals = sorted(
                    set(signals + named_calls(decorator, dependency_names))
                )
                if (
                    instance["dependencies"]
                    or mount["dependencies"]
                    or keyword(decorator, "dependencies") is not None
                ):
                    signals.append("declared_dependencies")
                endpoints.append(
                    {
                        "file": display,
                        "line": node.lineno,
                        "decorator_line": decorator.lineno,
                        "function": node.name,
                        "async": isinstance(node, ast.AsyncFunctionDef),
                        "method": method.upper(),
                        "path": full_path,
                        "declared_path": path,
                        "registration": {
                            "instance": owner,
                            "kind": instance["kind"],
                            "mount_owner": mount["owner"],
                            "mount_line": mount["line"],
                        },
                        "request_fields": request,
                        "response_fields": response,
                        "contract_candidate": {
                            "schema_version": 1,
                            "service": None,
                            "method_name": node.name,
                            "operation_id": None,
                            "request_message": None,
                            "response_message": None,
                        },
                        "authentication": {
                            "status": "review_required",
                            "dependency_signals": signals,
                        },
                        "side_effects": "unknown",
                        "shadow_execution": "blocked_until_review",
                        "blockers": sorted(set(blockers)),
                    }
                )
    return endpoints, diagnostics


def sources(root):
    diagnostics = []
    if root.is_symlink():
        raise ValueError("Input must not be a symbolic link.")
    if root.is_file():
        if root.suffix != ".py":
            raise ValueError("Input file must end in .py.")
        return [(root, root.name)], diagnostics
    if not root.is_dir():
        raise ValueError("Input must be an existing Python file or directory.")
    paths = []

    def walk_error(error):
        name = (
            Path(error.filename).relative_to(root).as_posix() if error.filename else "."
        )
        diagnostics.append(
            diagnostic(
                name,
                None,
                "directory_error",
                error.strerror or "Directory could not be read.",
            )
        )

    for directory, directories, files in os.walk(
        root, topdown=True, followlinks=False, onerror=walk_error
    ):
        directories.sort()
        files.sort()
        for name in directories:
            item = Path(directory) / name
            if item.is_symlink():
                diagnostics.append(
                    diagnostic(
                        item.relative_to(root).as_posix(),
                        None,
                        "skipped_symlink",
                        "Symbolic-link directory was not scanned.",
                    )
                )
        directories[:] = [
            name
            for name in directories
            if name not in IGNORED_DIRECTORIES
            and not (Path(directory) / name).is_symlink()
        ]
        for name in files:
            if not name.endswith(".py"):
                continue
            path = Path(directory) / name
            relative = path.relative_to(root).as_posix()
            if path.is_symlink():
                diagnostics.append(
                    diagnostic(
                        relative,
                        None,
                        "skipped_symlink",
                        "Symbolic-link source was not scanned.",
                    )
                )
                continue
            paths.append((path, relative))
            if len(paths) > MAX_SOURCE_FILES:
                raise ValueError("Project exceeds the 10000 Python file scan limit.")
    return paths, diagnostics


def analyze(root):
    paths, diagnostics = sources(root)
    endpoints = []
    for path, display in paths:
        found, problems = parse_file(path, display)
        endpoints.extend(found)
        diagnostics.extend(problems)
    routes = {}
    for endpoint in endpoints:
        if endpoint["path"] is not None:
            routes.setdefault((endpoint["method"], endpoint["path"]), []).append(
                endpoint
            )
    for duplicates in routes.values():
        if len(duplicates) > 1:
            for endpoint in duplicates:
                endpoint["blockers"].append(
                    "Another discovered route has the same method and path; review application scope and precedence."
                )
    endpoints.sort(
        key=lambda item: (
            item["file"],
            item["line"],
            item["decorator_line"],
            item["path"] or "",
        )
    )
    diagnostics.sort(key=lambda item: (item["file"], item["line"] or 0, item["code"]))
    return {
        "assessment_version": 1,
        "analysis": "static_ast_only",
        "scope": {
            "python_files_discovered": len(paths),
            "coverage": "partial",
            "exclusions": sorted(IGNORED_DIRECTORIES),
        },
        "limitations": [
            "Routes, dependencies, authentication, response behavior, and side effects can be dynamic or defined in other files.",
            "Candidate mappings require manual contract, compatibility, authentication, and behavioral review.",
        ],
        "endpoints": endpoints,
        "diagnostics": diagnostics,
        "migration_plan": {
            "status": "analysis_only",
            "steps": [
                "Resolve route registration, security, validation, errors, and side effects from application and deployment context.",
                "Review candidate fields and choose service, message names, and stable operation IDs before authoring a contract.",
                "Validate behavior using isolated fixtures and existing HTTP responses.",
                "Keep HTTP serving during any future opt-in rollout; define monitoring and rollback to HTTP before changing traffic.",
            ],
            "shadow_execution": "Only consider after side effects and operational impact are understood and controlled.",
        },
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "path", type=Path, help="Python source file or project directory"
    )
    args = parser.parse_args(argv)
    try:
        result = analyze(args.path)
    except (ValueError, OSError) as error:
        parser.exit(2, "assessment failed: " + str(error) + "\n")
    print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
