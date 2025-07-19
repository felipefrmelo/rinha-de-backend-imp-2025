from decimal import Decimal
from typing import Annotated
from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, Field


class PaymentRequest(BaseModel):
    """Client payment request - what comes from API"""
    correlationId: UUID
    amount: Annotated[Decimal, Field(gt=Decimal("0.00"))]


class PaymentProcessRequest(BaseModel):
    """Internal payment processing request - what goes to queue/processors"""
    correlationId: UUID
    amount: Annotated[Decimal, Field(gt=Decimal("0.00"))]
    requestedAt: datetime


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

