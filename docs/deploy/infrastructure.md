# Infrastructure

AWS architecture and Terraform structure.

## AWS Architecture

```mermaid
flowchart TB
    %% API Gateway is an AWS-managed edge service.
    %% In v1, API Gateway uses a private integration (VPC Link) to an internal ALB.
    subgraph Edge["AWS Edge / Managed"]
        APIGW[API Gateway]
    end

    subgraph AWS_Services["AWS Services"]
        EVENTBRIDGE[EventBridge Rules]
        SQS_QUEUES[SQS Queues]
        S3_BUCKET[S3 Data Bucket]
        S3_SCRATCH[S3 Scratch Bucket]
        ECR[ECR Repositories]
        CW[CloudWatch]
        SM[Secrets Manager]
        VPCE["VPC Endpoints\nS3/SQS/STS/KMS/Logs"]
    end

    subgraph VPC["VPC"]
        subgraph Private["Private Subnets"]
            ALB[Internal ALB]

            subgraph ECS["ECS Cluster"]
                DISPATCHER_SVC[Dispatcher Service]
                QUERY_SVC[Query Service]
                PLATFORM_WORKERS["Platform Workers - ecs_platform"]
                DELIVERY_SVC[Delivery Service]
                RPC_EGRESS[RPC Egress Gateway]
            end

            subgraph LAMBDA_VPC["VPC-attached Lambdas"]
                UDF_LAMBDA["Lambda UDF runner"]
                SOURCE_LAMBDA["Source/trigger Lambdas"]
            end

            RDS_STATE["RDS Postgres - state"]
            RDS_DATA["RDS Postgres - data"]
        end
    end

    APIGW -->|route| ALB
    ALB --> DISPATCHER_SVC
    ALB --> QUERY_SVC

    EVENTBRIDGE --> SOURCE_LAMBDA
    APIGW -->|webhooks (optional)| SOURCE_LAMBDA
    SOURCE_LAMBDA --> DISPATCHER_SVC

    DISPATCHER_SVC -->|invoke runtime=lambda| UDF_LAMBDA
    UDF_LAMBDA --> DISPATCHER_SVC
    UDF_LAMBDA --> QUERY_SVC

    DISPATCHER_SVC --> RDS_STATE
    DISPATCHER_SVC --> SQS_QUEUES
    DISPATCHER_SVC --> CW

    SQS_QUEUES --> PLATFORM_WORKERS

    PLATFORM_WORKERS --> DISPATCHER_SVC

    PLATFORM_WORKERS -->|hot data| RDS_DATA
    PLATFORM_WORKERS --> S3_BUCKET

    QUERY_SVC -->|read-only| RDS_DATA
    QUERY_SVC --> S3_BUCKET
    QUERY_SVC --> S3_SCRATCH


    PLATFORM_WORKERS --> RPC_EGRESS

    %% Secrets are injected at task launch via ECS task definition secrets.
    %% Untrusted UDF tasks do not have Secrets Manager permissions.

    ECR --> ECS
```

> Note: Untrusted ECS UDF execution (`ecs_udf`) is deferred to v2. In v1, untrusted UDFs execute via the platform-managed Lambda runner.


## Terraform Structure

```
/terraform
  /modules
    /vpc           # VPC, subnets, NAT, VPC endpoints
    /rds           # Postgres, security groups
    /ecs           # Cluster, services, task definitions, autoscaling
    /sqs           # SQS queues, DLQ
    /s3            # Data bucket, lifecycle rules
    /lambda        # Lambda functions (sources + operators), API Gateway
    /eventbridge   # Cron schedules
  /environments
    /dev
    /prod
```

## Key Resources

- **Ingress**: API Gateway validates user JWTs and routes to an **internal** ALB via VPC Link. Backend services must validate the user JWT and derive identity/role from it. Task-scoped endpoints (`/v1/task/*`) are internal-only and are not routed through the public Gateway. See `docs/architecture/containers/gateway.md` and `docs/standards/security_model.md`.
- **Lambda**: any Lambda that must call internal services (Dispatcher, Query Service, sinks) MUST be **VPC-attached** in private subnets with **no NAT**. Required AWS APIs are reached via VPC endpoints.
- **VPC**: Private/public subnets, VPC endpoints for S3/SQS (and other AWS APIs as needed)
- **ECS**: Fargate services, SQS-based autoscaling (v1 runs workers on `linux/amd64`)
- **RDS**: Two clusters/instances:
  - **Postgres state** for orchestration metadata
  - **Postgres data** for hot tables and platform-managed datasets
  Both are Postgres 15, encrypted, multi-AZ in prod, deployed into **private subnets**.
  
  For chain datasets, Postgres data should be optimized for frequent **block-range rewrites** (reorgs) and bounded deletes (post-compaction retention):
  - Baseline: bounded **row-range deletes** are supported. Large deletes can create bloat; tune autovacuum accordingly.
  - Optional optimization: **partition by `chain_id` + `block_number` range**. If partition boundaries align with compaction ranges, retention cleanup can later be implemented as partition drops.
  - Retention and compaction are **DAG-defined behaviors** (operators decide finality/TTL); the Dispatcher does not enforce a retention policy.
  - In prod, consider a read replica for Query Service to protect ingestion latency.
- **SQS**: Standard queues (one per runtime) + DLQ. Base visibility is minutes; worker wrappers extend visibility for long tasks. Ordering is enforced by DAG dependencies, not SQS.
- **S3**: Data bucket for dataset storage + scratch bucket for query exports and task scratch
- **Query Service**: DuckDB federation layer (read-only Postgres user) + result export to S3
- **Dispatcher credential minting**: Issues short-lived, prefix-scoped STS credentials for untrusted UDF tasks


## Scheduled and webhook triggers

- **Cron / schedules**: use EventBridge Rules (or EventBridge Scheduler) to invoke a Lambda function on a schedule.
  The scheduled Lambda can enqueue work by calling the Dispatcher or publishing to SQS.
- **Webhooks**: use API Gateway to invoke Lambda (or forward to Gateway/Dispatcher).
  API Gateway is the recommended entry point when you want auth, rate limiting, and request validation.

## Deployment Order

1. Terraform apply (infra)
2. Database migrations
3. Sync DAG YAML → Postgres
4. Deploy ECS services

## Rollback

Terraform state rollback, ECS deployment rollback, git revert DAGs.
