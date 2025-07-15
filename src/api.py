from datetime import datetime
from typing import Annotated, Optional

from fastapi import FastAPI, Query

from src.models import PaymentRequest, PaymentsSummary, ProcessorSummary
from src.services import PaymentProcessor, PaymentService, PaymentStorage


def create_app(
    default_processor: PaymentProcessor,
    fallback_processor: PaymentProcessor,
    storage: PaymentStorage,
) -> FastAPI:
    app = FastAPI()
    payment_service = PaymentService(
        default_processor=default_processor,
        fallback_processor=fallback_processor,
        storage=storage,
    )

    @app.post("/payments")
    async def process_payment(payment_request: PaymentRequest):
        result = await payment_service.process_payment(payment_request)
        return result

    @app.get("/payments-summary", response_model=PaymentsSummary)
    async def payments_summary(
        from_: Annotated[
            Optional[datetime],
            Query(
                alias="from",
                description="Start datetime in ISO format",
                examples=["2024-01-01T00:00:00Z", "2024-01-01T00:00:00"],
            ),
        ] = None,
        to: Annotated[
            Optional[datetime],
            Query(
                description="End datetime in ISO format", 
                examples=["2024-12-31T23:59:59Z", "2024-12-31T23:59:59"],
            ),
        ] = None,
    ):
        # Use datetime parameters directly - FastAPI handles parsing automatically
        from_timestamp = from_ if from_ else datetime.min
        to_timestamp = to if to else datetime.max

        # Get summary from storage
        summary_data = await storage.get_payments_summary(from_timestamp, to_timestamp)

        return PaymentsSummary(
            default=ProcessorSummary(
                totalRequests=summary_data.get("default", {}).get("totalRequests", 0),
                totalAmount=summary_data.get("default", {}).get("totalAmount", 0),
            ),
            fallback=ProcessorSummary(
                totalRequests=summary_data.get("fallback", {}).get("totalRequests", 0),
                totalAmount=summary_data.get("fallback", {}).get("totalAmount", 0),
            ),
        )

    return app
