import os
import redis.asyncio as redis
from src.domain.services import PaymentService, PaymentProvider
from src.adapters.storage import RedisPaymentStorage
from src.adapters.http import HttpPaymentProcessor, HttpxHttpClient
from src.adapters.cache import CacheProxy
from src.domain.health_check import HealthCheckClient


def create_payment_service() -> PaymentService:
    """Create a PaymentService with real implementations for production use."""
    
    # Environment-aware Redis configuration
    redis_host = os.environ.get('REDIS_HOST', 'redis')  # Default to 'redis' for production
    redis_client = redis.Redis(
        host=redis_host,
        port=6379,
        db=0,
        decode_responses=True
    )
    storage = RedisPaymentStorage(redis_client=redis_client)
    
    # Create shared HTTP client and cache
    http_client = HttpxHttpClient()
    redis_url = f"redis://{redis_host}:6379"
    cache = CacheProxy(redis_url)
    
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
