from contextlib import asynccontextmanager
from datetime import datetime
from typing import Annotated

from fastapi import FastAPI, Query, Request
from fastapi.responses import HTMLResponse
from pyinstrument import Profiler

from src.domain.models import PaymentRequest, PaymentsSummary, ProcessorSummary
from src.domain.services import PaymentService


def create_app(
    payment_service: PaymentService,
) -> FastAPI:
    
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        # Start the background health monitoring
        health_service = payment_service.default.health_check
        await health_service.start()
        yield
        # Stop the background health monitoring
        await health_service.stop()
    
    app = FastAPI(lifespan=lifespan)

    @app.middleware("http")
    async def profile_request(request: Request, call_next):
        profiling = request.query_params.get("profile", False)
        if profiling:
            profiler = Profiler(interval=0.0001)
            profiler.start()
            response = await call_next(request)
            profiler.stop()
            return HTMLResponse(profiler.output_html())
        else:
            return await call_next(request)

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
                totalAmount=summary_data.get("default", {}).get("totalAmount", 0.0),
            ),
            fallback=ProcessorSummary(
                totalRequests=summary_data.get("fallback", {}).get("totalRequests", 0),
                totalAmount=summary_data.get("fallback", {}).get("totalAmount", 0.0),
            ),
        )

    return app
