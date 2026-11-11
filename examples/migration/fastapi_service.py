"""Example source for the static assessment. Running it requires FastAPI."""

from typing import Annotated

from fastapi import APIRouter, Depends, FastAPI
from pydantic import BaseModel


class User(BaseModel):
    name: str
    active: bool


def require_user():
    raise NotImplementedError("Replace with the application's authentication")


app = FastAPI()
users = APIRouter(prefix="/users")


@users.get("/{user_id}", response_model=User)
def get_user(user_id: str, user: Annotated[str, Depends(require_user)]):
    raise NotImplementedError("Replace with the application's lookup")


app.include_router(users, prefix="/v1")
