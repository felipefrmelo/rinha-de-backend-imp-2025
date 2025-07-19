import os
import redis.asyncio as redis
from src.domain.services import PaymentService, PaymentProvider
from src.domain.queue_manager import QueueManager
from src.domain.payment_worker import PaymentWorker
from src.domain.background_worker import BackgroundWorker
from src.adapters.storage import RedisPaymentStorage
from src.adapters.http import HttpPaymentProcessor, HttpxHttpClient
from src.adapters.cache import CacheProxy
from src.adapters.redis_client import RedisQueueClient
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


def create_queue_manager() -> QueueManager:
    """Create a QueueManager with Redis implementation for production use."""
    
    # Environment-aware Redis configuration
    redis_host = os.environ.get('REDIS_HOST', 'redis')  # Default to 'redis' for production
    redis_client = redis.Redis(
        host=redis_host,
        port=6379,
        db=0,  # Use same DB as storage for simplicity
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
    
    queue_client = RedisQueueClient(redis_connection=redis_client)
    return QueueManager(queue_client=queue_client)


def create_background_worker(payment_service: PaymentService, queue_manager: QueueManager) -> BackgroundWorker:
    """Create a BackgroundWorker with PaymentWorker for production use."""
    
    # Create payment worker that processes from queue
    payment_worker = PaymentWorker(
        queue_manager=queue_manager,
        payment_service=payment_service
    )
    
    # Create background worker with reasonable poll interval
    return BackgroundWorker(
        payment_worker=payment_worker,
        poll_interval=0.1  # 100ms polling interval
    )
