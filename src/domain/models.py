from decimal import Decimal
from typing import Annotated
from uuid import UUID

from pydantic import BaseModel, Field


class PaymentRequest(BaseModel):
    correlationId: UUID
    amount: Annotated[Decimal, Field(gt=Decimal("0.00"))]


class PaymentResponse(BaseModel):
    message: str


class ProcessorSummary(BaseModel):
    totalRequests: int
    totalAmount: float


class PaymentsSummary(BaseModel):
    default: ProcessorSummary
    fallback: ProcessorSummary


class HealthStatus(BaseModel):
    failing: bool
    min_response_time: int

