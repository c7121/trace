# Infrastructure

AWS architecture and Terraform structure.

## AWS Architecture

```mermaid
flowchart TB
    %% NOTE: API Gateway is an AWS-managed edge service, not deployed into subnets.
    %% If private integration is desired, use API Gateway VPC Link -> ALB/NLB.
    subgraph Edge["AWS Edge / Managed"]
        APIGW[API Gateway]
    end

    subgraph VPC["VPC"]
        subgraph Public["Public Subnets"]
            ALB[Application Load Balancer]
        end

        subgraph Private["Private Subnets"]
            subgraph ECS["ECS Cluster"]
                DISPATCHER_SVC[Dispatcher Service]
                QUERY_SVC[Query Service]
                BROKER_SVC[Credential Broker]
                RUST_WORKERS["Rust Workers - ecs_rust"]
                PYTHON_WORKERS["Python Workers - ecs_python"]
                UDF_WORKERS["UDF Workers - ecs_udf_*"]
                DELIVERY_SVC[Delivery Service]
                SINKS_SVC[Dataset Sinks]
                RPC_EGRESS[RPC Egress Gateway]
            end

            RDS_STATE["RDS Postgres - state"]
            RDS_DATA["RDS Postgres - data"]
        end
    end

    subgraph Serverless["Serverless"]
        EVENTBRIDGE[EventBridge Rules]
        LAMBDA[Lambda Functions]
    end
    
    subgraph AWS_Services["AWS Services"]
        SQS_QUEUES[SQS Queues]
        S3_BUCKET[S3 Data Bucket]
        S3_SCRATCH[S3 Scratch Bucket]
        ECR[ECR Repositories]
        CW[CloudWatch]
        SM[Secrets Manager]
    end
    
    EVENTBRIDGE --> LAMBDA
    APIGW --> LAMBDA
    LAMBDA --> DISPATCHER_SVC
    DISPATCHER_SVC -->|invoke runtime=lambda| LAMBDA
    
    ALB --> DISPATCHER_SVC
    
    DISPATCHER_SVC --> RDS_STATE
    DISPATCHER_SVC --> SQS_QUEUES
    DISPATCHER_SVC --> CW
    
    SQS_QUEUES --> RUST_WORKERS
    SQS_QUEUES --> PYTHON_WORKERS
    SQS_QUEUES --> UDF_WORKERS
    
    RUST_WORKERS --> DISPATCHER_SVC
    PYTHON_WORKERS --> DISPATCHER_SVC
    UDF_WORKERS --> DISPATCHER_SVC
    
    RUST_WORKERS -->|hot data| RDS_DATA
    RUST_WORKERS --> S3_BUCKET
    PYTHON_WORKERS -->|hot data| RDS_DATA
    PYTHON_WORKERS --> S3_BUCKET

    QUERY_SVC -->|read-only| RDS_DATA
    QUERY_SVC --> S3_BUCKET
    QUERY_SVC --> S3_SCRATCH

    UDF_WORKERS --> QUERY_SVC
    UDF_WORKERS --> BROKER_SVC
    UDF_WORKERS --> S3_BUCKET
    UDF_WORKERS --> SQS_QUEUES

    RUST_WORKERS --> RPC_EGRESS
    PYTHON_WORKERS --> RPC_EGRESS
    
    %% Secrets are injected at task launch via ECS task definition secrets.
    %% Untrusted UDF tasks do not have Secrets Manager permissions.
    
    ECR --> ECS
```

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
- **Credential Broker**: Issues short-lived, prefix-scoped STS credentials for untrusted UDF tasks

## Deployment Order

1. Terraform apply (infra)
2. Database migrations
3. Sync DAG YAML → Postgres
4. Deploy ECS services

## Rollback

Terraform state rollback, ECS deployment rollback, git revert DAGs.
