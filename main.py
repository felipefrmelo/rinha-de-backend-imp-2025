from fastapi import FastAPI
from src.api import create_app
from src.factories import create_payment_service

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
    uvicorn.run(app, host="0.0.0.0", port=9999)


if __name__ == "__main__":
    main()
