#!/bin/bash

# Script para reiniciar containers e rodar testes da Rinha de Backend 2025

set -e  # Exit on any error

echo "🔄 Parando containers do backend..."
docker compose down

echo "🔄 Parando containers dos payment processors..."
cd payment-processor
docker compose down
cd ..

echo "📦 Rebuilding containers do backend..."
docker compose build

echo "🚀 Iniciando payment processors..."
cd payment-processor
docker compose up -d
cd ..

echo "⏳ Aguardando payment processors ficarem prontos..."
sleep 5

echo "🏥 Verificando saúde dos payment processors..."
for i in {1..30}; do
    if curl -f http://localhost:8001/payments/service-health >/dev/null 2>&1 && \
       curl -f http://localhost:8002/payments/service-health >/dev/null 2>&1; then
        echo "✅ Payment processors estão saudáveis!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Payment processors não ficaram saudáveis após 30 tentativas"
        exit 1
    fi
    echo "⏳ Tentativa $i/30 - aguardando payment processors..."
    sleep 2
done

echo "🚀 Iniciando containers do backend..."
docker compose up -d

echo "⏳ Aguardando backend ficar pronto..."
sleep 5

echo "🧹 Limpando cache Redis..."
docker exec rinha-de-backend-imp-2025-redis-1 redis-cli FLUSHALL || echo "⚠️ Aviso: Não foi possível limpar Redis cache"

echo "🏥 Verificando saúde do backend..."
for i in {1..30}; do
    if curl -f http://localhost:9999/health >/dev/null 2>&1; then
        echo "✅ Backend está saudável!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Backend não ficou saudável após 30 tentativas"
        exit 1
    fi
    echo "⏳ Tentativa $i/30 - aguardando backend..."
    sleep 1
done

echo "🧪 Executando testes de performance com k6..."
cd rinha-test

# Configurar dashboard do k6
ENABLE_K6_DASHBOARD=${ENABLE_K6_DASHBOARD:-false}
if [ "${ENABLE_K6_DASHBOARD}" = "true" ]; then
    export K6_WEB_DASHBOARD=true
    export K6_WEB_DASHBOARD_OPEN=true
    export K6_WEB_DASHBOARD_PORT=5665
    export K6_WEB_DASHBOARD_PERIOD=2s
    export K6_WEB_DASHBOARD_EXPORT='report.html'

    echo "📄 Relatório será salvo em: report.html"
    echo "📊 Dashboard k6 disponível em: http://localhost:5665"
else
    export K6_WEB_DASHBOARD=false
    export K6_WEB_DASHBOARD_OPEN=false
fi

export MAX_REQUESTS=500


# Executar teste com configurações otimizadas
k6 run --quiet rinha.js

echo "✅ Testes concluídos!"
echo "📊 Verifique os resultados acima para métricas de performance"
