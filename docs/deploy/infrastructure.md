# Infrastructure

AWS architecture and Terraform structure.

## AWS Architecture

```mermaid
flowchart TB
    %% NOTE: API Gateway is an AWS-managed edge service (not deployed into subnets).
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
                RUST_WORKERS[Rust Workers (ecs_rust)]
                PYTHON_WORKERS[Python Workers (ecs_python)]
                INGEST_WORKERS[Ingest Workers]
            end

            RDS[(RDS Postgres)]
        end
    end

    subgraph Serverless["Serverless"]
        EVENTBRIDGE[EventBridge Rules]
        LAMBDA[Lambda Functions]
    end
    
    subgraph AWS_Services["AWS Services"]
        SQS_QUEUES[SQS Queues]
        S3_BUCKET[S3 Data Bucket]
        ECR[ECR Repositories]
        CW[CloudWatch]
        SM[Secrets Manager]
    end
    
    EVENTBRIDGE --> LAMBDA
    APIGW --> LAMBDA
    LAMBDA --> DISPATCHER_SVC
    DISPATCHER_SVC -->|invoke runtime=lambda| LAMBDA
    
    ALB --> DISPATCHER_SVC
    
    DISPATCHER_SVC --> RDS
    DISPATCHER_SVC --> SQS_QUEUES
    DISPATCHER_SVC --> CW
    
    SQS_QUEUES --> RUST_WORKERS
    SQS_QUEUES --> PYTHON_WORKERS
    SQS_QUEUES --> INGEST_WORKERS
    
    RUST_WORKERS --> DISPATCHER_SVC
    PYTHON_WORKERS --> DISPATCHER_SVC
    INGEST_WORKERS --> DISPATCHER_SVC
    
    RUST_WORKERS -->|hot data| RDS
    RUST_WORKERS --> S3_BUCKET
    PYTHON_WORKERS -->|hot data| RDS
    PYTHON_WORKERS --> S3_BUCKET
    INGEST_WORKERS -->|hot data| RDS
    
    RUST_WORKERS --> SM
    INGEST_WORKERS --> SM
    
    ECR --> ECS
```

## Terraform Structure

```
/terraform
  /modules
    /vpc           # VPC, subnets, NAT, VPC endpoints
    /rds           # Postgres, security groups
    /ecs           # Cluster, services, task definitions, autoscaling
    /sqs           # FIFO queues, DLQ
    /s3            # Data bucket, lifecycle rules
    /lambda        # Lambda functions (sources + operators), API Gateway
    /eventbridge   # Cron schedules
  /environments
    /dev
    /prod
```

## Key Resources

- **VPC**: Private/public subnets, VPC endpoints for S3/SQS/Secrets Manager (and other AWS APIs as needed)
- **ECS**: Fargate services, SQS-based autoscaling (v1 runs workers on `linux/amd64`)
- **RDS**: Postgres 15, encrypted, multi-AZ in prod. Deployed into **private subnets** with no public accessibility.
- **SQS**: FIFO with deduplication, 5min visibility, DLQ after 3 failures
- **S3**: Versioned, lifecycle to Glacier after 1 year

## Deployment Order

1. Terraform apply (infra)
2. Database migrations
3. Sync DAG YAML → Postgres
4. Deploy ECS services

## Rollback

Terraform state rollback, ECS deployment rollback, git revert DAGs.
