from fastapi import FastAPI

from src.models import PaymentRequest
from src.services import PaymentService, PaymentProcessor


def create_app(default_processor: PaymentProcessor) -> FastAPI:
    app = FastAPI()
    payment_service = PaymentService(default_processor=default_processor)
    
    @app.post("/payments")
    def process_payment(payment_request: PaymentRequest):
        result = payment_service.process_payment(payment_request)
        return result
    
    return app