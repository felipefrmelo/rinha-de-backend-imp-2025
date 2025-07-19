import asyncio
import logging
from typing import Optional

from src.domain.payment_worker import PaymentWorker

logger = logging.getLogger(__name__)


class BackgroundWorker:
    """Background worker that continuously processes payments from the queue."""
    
    def __init__(self, payment_worker: PaymentWorker, poll_interval: float = 0.1):
        self.payment_worker = payment_worker
        self.poll_interval = poll_interval
        self._task: Optional[asyncio.Task] = None
        self._running = False
    
    def start(self) -> None:
        """Start the background worker loop."""
        if self._running:
            return
            
        self._running = True
        self._task = asyncio.create_task(self._worker_loop())
        logger.info("Background worker started")
    
    async def stop(self) -> None:
        """Stop the background worker loop."""
        if not self._running:
            return
            
        self._running = False
        
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
                
        logger.info("Background worker stopped")
    
    def is_running(self) -> bool:
        """Check if the background worker is running."""
        return self._running
    
    async def _worker_loop(self) -> None:
        """Main worker loop that continuously processes payments."""
        try:
            while self._running:
                try:
                    # Try to process the next payment
                    processed = await self.payment_worker.process_next_payment()
                    
                    await asyncio.sleep(self.poll_interval)
                    
                except Exception as e:
                    logger.error(f"Error in background worker loop: {e}")
                    # Continue running even if there was an error
                    await asyncio.sleep(self.poll_interval)
                    
        except asyncio.CancelledError:
            logger.debug("Background worker loop cancelled")
        finally:
            self._running = False
