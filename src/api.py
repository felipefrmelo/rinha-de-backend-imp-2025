from datetime import datetime
from typing import Annotated

from fastapi import FastAPI, Query

from src.domain.models import PaymentRequest, PaymentsSummary, ProcessorSummary
from src.domain.services import PaymentService


def create_app(
    payment_service: PaymentService,
) -> FastAPI:
    app = FastAPI()

    @app.post("/payments")
    async def process_payment(payment_request: PaymentRequest):
        result = await payment_service.process_payment(payment_request)
        return result

    @app.get("/payments-summary", response_model=PaymentsSummary)
    async def payments_summary(
        from_: Annotated[
            datetime,
            Query(alias="from"),
        ] = datetime.min,
        to: Annotated[
            datetime,
            Query(),
        ] = datetime.max,
    ):
        summary_data = await payment_service.get_payments_summary(from_, to)

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
