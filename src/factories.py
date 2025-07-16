from src.domain.services import PaymentService, PaymentProvider
from src.adapters.storage import InMemoryPaymentStorage
from src.adapters.http import HttpPaymentProcessor, HttpxHttpClient
from src.adapters.cache import InMemoryHealthStatusCache
from src.domain.health_check import HealthCheckClient


def create_payment_service() -> PaymentService:
    """Create a PaymentService with real implementations for production use."""
    
    # Create real implementations for production
    # For now, we'll use in-memory storage - this should be replaced with a database
    storage = InMemoryPaymentStorage()
    
    # Create shared HTTP client and cache
    http_client = HttpxHttpClient()
    cache = InMemoryHealthStatusCache()
    
    # Create health check clients for both processors
    default_health_check = HealthCheckClient("http://payment-processor-default:8080", http_client, cache)
    fallback_health_check = HealthCheckClient("http://payment-processor-fallback:8080", http_client, cache)
    
    # Create payment processors
    default_processor = HttpPaymentProcessor("http://payment-processor-default:8080")
    fallback_processor = HttpPaymentProcessor("http://payment-processor-fallback:8080")
    
    # Create PaymentProvider objects
    default_provider = PaymentProvider(
        processor=default_processor,
        health_check=default_health_check,
        name="default"
    )
    
    fallback_provider = PaymentProvider(
        processor=fallback_processor,
        health_check=fallback_health_check,
        name="fallback"
    )
    
    return PaymentService(
        default=default_provider,
        fallback=fallback_provider,
        storage=storage,
    )
