import asyncio
from typing import Optional
import logging

logger = logging.getLogger(__name__)

# Custom exceptions
class QueueError(Exception):
    """Raised when queue operations fail."""
    pass

class QueueConnectionError(QueueError):
    """Raised when queue connection fails."""
    pass


class RedisQueueClient:
    """Redis-based implementation of QueueClient for production."""
    
    def __init__(self, redis_connection):
        self.redis_connection = redis_connection
    
    async def enqueue(self, queue_name: str, message: dict) -> None:
        """Enqueue a message to Redis stream."""
        try:
            logger.debug(f"Enqueuing message to {queue_name}")
            await self.redis_connection.xadd(queue_name, message)
            logger.debug(f"Message enqueued to {queue_name} successfully")
        except Exception as e:
            logger.error(f"Queue error for {queue_name}: {e}")
            raise QueueError(f"Failed to enqueue to {queue_name}: {e}") from e
    
    async def dequeue(self, queue_name: str, timeout_ms: int = 1000) -> Optional[dict]:
        """Dequeue a message from Redis stream."""
        try:
            logger.debug(f"Dequeuing message from {queue_name}")
            
            # Use XREAD to get messages from stream starting from beginning
            result = await self.redis_connection.xread(
                {queue_name: "0"}, 
                count=1, 
                block=timeout_ms
            )
            
            if not result:
                return None
                
            # result format: [[stream_name, [[msg_id, fields]]]]
            stream_data = result[0][1]  # Get messages from first stream
            if not stream_data:
                return None
                
            message_id, fields = stream_data[0]  # Get first message
            logger.debug(f"Message dequeued from {queue_name}: {message_id}")
            
            # Delete the message after reading to simulate queue behavior
            await self.redis_connection.xdel(queue_name, message_id)
            
            return fields
            
        except Exception as e:
            logger.error(f"Failed to dequeue from {queue_name}: {e}")
            raise QueueConnectionError(f"Redis connection error: {e}") from e