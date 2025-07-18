import os
import redis.asyncio as redis
from src.domain.services import PaymentService, PaymentProvider
from src.adapters.storage import RedisPaymentStorage
from src.adapters.http import HttpPaymentProcessor, HttpxHttpClient
from src.adapters.cache import CacheProxy
from src.domain.health_check import HealthCheckClient


def create_payment_service() -> PaymentService:
    """Create a PaymentService with real implementations for production use."""
    
    # Environment-aware Redis configuration with connection pooling
    redis_host = os.environ.get('REDIS_HOST', 'redis')  # Default to 'redis' for production
    redis_client = redis.Redis(
        host=redis_host,
        port=6379,
        db=0,
        decode_responses=True,
        max_connections=50,
        connection_pool=redis.ConnectionPool(
            host=redis_host,
            port=6379,
            db=0,
            decode_responses=True,
            max_connections=50
        )
    )
    storage = RedisPaymentStorage(redis_client=redis_client)
    
    # Create shared HTTP client and cache
    http_client = HttpxHttpClient()
    redis_url = f"redis://{redis_host}:6379"
    cache = CacheProxy(redis_url)
    
    # Create background health check service for both processors
    processor_urls = {
        "default": "http://payment-processor-default:8080",
        "fallback": "http://payment-processor-fallback:8080"
    }
    health_service = HealthCheckClient(processor_urls, http_client, cache)
    
    # Create payment processors
    default_processor = HttpPaymentProcessor("http://payment-processor-default:8080")
    fallback_processor = HttpPaymentProcessor("http://payment-processor-fallback:8080")
    
    # Create PaymentProvider objects
    default_provider = PaymentProvider(
        processor=default_processor,
        health_check=health_service,
        name="default"
    )
    
    fallback_provider = PaymentProvider(
        processor=fallback_processor,
        health_check=health_service,
        name="fallback"
    )
    
    return PaymentService(
        default=default_provider,
        fallback=fallback_provider,
        storage=storage,
    )
