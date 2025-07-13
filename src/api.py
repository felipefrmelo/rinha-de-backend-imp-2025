from fastapi import FastAPI

from src.models import PaymentRequest
from src.services import PaymentProcessor, PaymentService, PaymentStorage


def create_app(
    default_processor: PaymentProcessor,
    fallback_processor: PaymentProcessor,
    storage: PaymentStorage,
) -> FastAPI:
    app = FastAPI()
    payment_service = PaymentService(
        default_processor=default_processor, fallback_processor=fallback_processor, storage=storage
    )

    @app.post("/payments")
    async def process_payment(payment_request: PaymentRequest):
        result = await payment_service.process_payment(payment_request)
        return result

    return app

