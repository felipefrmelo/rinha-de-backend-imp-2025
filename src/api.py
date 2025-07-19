from contextlib import asynccontextmanager
from datetime import datetime
from typing import Annotated
import logging
import json
from decimal import Decimal

from fastapi import FastAPI, Query, Request, HTTPException
from fastapi.responses import HTMLResponse, JSONResponse
from fastapi.exceptions import RequestValidationError
from fastapi.encoders import jsonable_encoder
from pyinstrument import Profiler

from src.domain.models import PaymentRequest, PaymentsSummary, ProcessorSummary
from src.domain.services import PaymentService
from src.domain.queue_manager import QueueManager
from src.domain.background_worker import BackgroundWorker

logger = logging.getLogger(__name__)


def create_app(
    payment_service: PaymentService,
    queue_manager: QueueManager,
    background_worker: BackgroundWorker,
) -> FastAPI:
    
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        # Start the background health monitoring
        health_service = payment_service.default.health_check
        await health_service.start()
        
        background_worker.start()
        logger.info("Background payment worker started")
        
        yield
        
        await background_worker.stop()
        logger.info("Background payment worker stopped")
            
        # Stop the background health monitoring
        await health_service.stop()
    
    app = FastAPI(lifespan=lifespan)

    # Global exception handlers
    @app.exception_handler(Exception)
    async def general_exception_handler(request: Request, exc: Exception):
        logger.error(f"Unexpected error in {request.method} {request.url.path}: {str(exc)}")
        return JSONResponse(
            status_code=500,
            content={"error": "Internal server error", "detail": "An unexpected error occurred"}
        )

    @app.exception_handler(RequestValidationError)
    async def validation_exception_handler(request: Request, exc: RequestValidationError):
        logger.warning(f"Validation error in {request.method} {request.url.path}: {exc.errors()}")
        # Use jsonable_encoder to handle Decimal and other non-JSON serializable objects
        return JSONResponse(
            status_code=400,
            content=jsonable_encoder({"error": "Validation error", "detail": exc.errors()})
        )

    @app.exception_handler(HTTPException)
    async def http_exception_handler(request: Request, exc: HTTPException):
        logger.warning(f"HTTP error in {request.method} {request.url.path}: {exc.detail}")
        return JSONResponse(
            status_code=exc.status_code,
            content={"error": exc.detail}
        )

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
        try:
                await queue_manager.add_payment_to_queue(payment_request)
                return JSONResponse(
                    status_code=202,
                    content={
                        "message": "Payment accepted for processing",
                        "correlationId": str(payment_request.correlationId)
                    }
                )
        except Exception as e:
            logger.error(f"Payment processing failed for {payment_request.correlationId}: {str(e)}")
            raise HTTPException(
                status_code=503,
                detail="Payment processing temporarily unavailable"
            )

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
        try:
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
        except Exception as e:
            logger.error(f"Failed to retrieve payments summary: {str(e)}")
            raise HTTPException(
                status_code=500,
                detail="Unable to retrieve payments summary"
            )

    @app.get("/health")
    async def health_check():
        """Simple health check endpoint for load balancer."""
        return {"status": "healthy"}

    return app
