# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a repository for the **Rinha de Backend 2025** competition - a backend challenge to build a payment processing intermediary service. The goal is to create a backend that intermediates payment requests between clients and two payment processors (default and fallback), optimizing for the lowest fees while handling service instabilities.

## Challenge Architecture

The backend must:
- Receive payment requests via `POST /payments`
- Forward payments to either:
  - Payment Processor Default (`http://payment-processor-default:8080`) - lower fees
  - Payment Processor Fallback (`http://payment-processor-fallback:8080`) - higher fees, used when default fails
- Provide payment summaries via `GET /payments-summary`
- Handle service instabilities and optimize for minimal fees
- Achieve performance targets (p99 < 11ms for bonus scoring)

## Required Endpoints

### Your Backend Must Implement:
- `POST /payments` - Accept payment requests with `correlationId` (UUID) and `amount` (decimal)
- `GET /payments-summary?from=<ISO_timestamp>&to=<ISO_timestamp>` - Return summary with `default` and `fallback` totals

### Payment Processors Provide:
- `POST /payments` - Process payments (requires `correlationId`, `amount`, `requestedAt`)
- `GET /payments/service-health` - Check service status (rate limited: 1 call per 5 seconds)
- Various admin endpoints for testing (require `X-Rinha-Token` header, default token: "123")

## Docker Requirements

- Must use `docker-compose.yml` with services exposed on port 9999
- CPU limit: 1.5 total across all services
- Memory limit: 350MB total across all services
- Must include `payment-processor` network configuration
- All images must be publicly available and linux-amd64 compatible

Example network configuration:
```yml
networks:
  payment-processor:
    external: true
```

## Development Setup

Since this is a new project repository (only contains INSTRUCOES.md), you'll need to:

1. **First, start the payment processors:**
   ```bash
   # In the payment-processor directory (not included in this repo)
   docker-compose up -d
   ```

2. **Then start your backend:**
   ```bash
   docker-compose up -d
   ```

3. **Access points:**
   - Your backend: `http://localhost:9999`
   - Payment Processor Default: `http://localhost:8001`
   - Payment Processor Fallback: `http://localhost:8002`

4. **Testing:**
   - Follow instructions in `rinha-test/README.md` for local testing
   - Use admin endpoints to simulate failures and delays during development

## Scoring Criteria

- **Primary:** Profit maximization (more payments with lower fees = better score)
- **Penalty:** 35% of total profit for consistency violations between your summary and processors
- **Bonus:** Performance bonus for p99 < 11ms: `(11 - p99) * 0.02` percentage bonus

## Current Repository State

This repository currently contains only the instructions file. You'll need to implement:
- Backend service code
- Docker configuration
- Load balancer setup (minimum 2 web server instances required)
- Database/storage solution for tracking payments
- Logic for choosing between default/fallback processors

## Submission Requirements

- Public git repository with source code
- `docker-compose.yml` in participant directory
- `README.md` explaining technologies used
- `info.json` with metadata about technologies used
- Deadline: 2025-08-17 23:59:59