from src.config.logging import setup_logging
from src.api import create_app
from src.factories import create_payment_service

# Setup logging first
setup_logging()

# Create the payment service
payment_service = create_payment_service()

# Create the FastAPI app with the payment service
app = create_app(payment_service)

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
