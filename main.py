from src.config.logging import setup_logging
from src.api import create_app
from src.factories import create_payment_service, create_queue_manager, create_background_worker

# Setup logging first
setup_logging()

# Create the payment service and queue manager
payment_service = create_payment_service()
queue_manager = create_queue_manager()

# Create the background worker
background_worker = create_background_worker(payment_service, queue_manager)

# Create the FastAPI app with all components
app = create_app(payment_service, queue_manager, background_worker)

# Add health check endpoint
@app.get("/health")
async def health_check():
    return {"status": "healthy"}


def main():
    import uvicorn
    from src.config.logging import get_uvicorn_log_level
    
    # Get log level for uvicorn
    log_level = get_uvicorn_log_level()
    
    uvicorn.run(
        app, 
        host="0.0.0.0", 
        port=9999, 
        log_level=log_level
    )


if __name__ == "__main__":
    main()
