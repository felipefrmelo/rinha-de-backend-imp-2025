# Rinha de Backend - 2025

Este é o script de teste de carga para a Rinha de Backend 2025.

## Como executar

1. Instale o k6 seguindo o guia oficial de instalação
2. Inicie os containers do backend e dos processadores de pagamento
3. Navegue até o diretório "rinha-test"
4. Execute os testes com o comando: `k6 run rinha.js`

## Dashboard e Relatórios

Você pode configurar variáveis de ambiente opcionais para habilitar o dashboard web:

```shell
export K6_WEB_DASHBOARD=true
export K6_WEB_DASHBOARD_PORT=5665
```

## Personalização

Ajuste o número máximo de requisições simultâneas usando a variável `MAX_REQUESTS`:

```shell
k6 run -e MAX_REQUESTS=850 rinha.js
```

## Contribuições

Este script de teste foi criado por Zan e aceita contribuições e melhorias via pull requests.

O documento enfatiza o envolvimento da comunidade e fornece orientação clara e passo a passo para executar os testes de performance do backend.