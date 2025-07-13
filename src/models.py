from decimal import Decimal
from typing import Annotated
from uuid import UUID

from pydantic import BaseModel, Field


class PaymentRequest(BaseModel):
    correlationId: UUID
    amount: Annotated[Decimal, Field(gt=Decimal("0.00"))]
