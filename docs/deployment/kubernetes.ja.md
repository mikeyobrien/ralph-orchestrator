# Kubernetes デプロイガイド

スケーラブルで回復力のある AI オーケストレーションのために、Kubernetes 上に Ralph
Orchestrator をデプロイします。

## 前提条件

- Kubernetes クラスタ 1.20 以降（ローカルまたはクラウド）
- クラスタアクセスで設定された `kubectl`
- Helm 3.0 以降（任意、Helm デプロイ用）
- コンテナレジストリへのアクセス（Docker Hub、GCR、ECR など）
- それぞれ 4GB RAM を持つ最低 2 ノード

## クイックスタート

### kubectl での基本的なデプロイ

名前空間を作成してデプロイします。

```bash
# 名前空間を作成する
kubectl create namespace ralph-orchestrator

# マニフェストを適用する
kubectl apply -f k8s/ -n ralph-orchestrator

# デプロイを確認する
kubectl get pods -n ralph-orchestrator
```

## Kubernetes マニフェスト

### 1. Namespace と ConfigMap

```yaml
# k8s/00-namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: ralph-orchestrator
---
# k8s/01-configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ralph-config
  namespace: ralph-orchestrator
data:
  RALPH_AGENT: "auto"
  RALPH_MAX_ITERATIONS: "100"
  RALPH_MAX_RUNTIME: "14400"
  RALPH_CHECKPOINT_INTERVAL: "5"
  RALPH_VERBOSE: "true"
  RALPH_ENABLE_METRICS: "true"
```

### 2. シークレットの管理

```yaml
# k8s/02-secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: ralph-secrets
  namespace: ralph-orchestrator
type: Opaque
stringData:
  CLAUDE_API_KEY: "sk-ant-..."
  GEMINI_API_KEY: "AIza..."
  Q_API_KEY: "..."
```

コマンドラインからシークレットを適用します。

```bash
# リテラルからシークレットを作成する
kubectl create secret generic ralph-secrets \
  --from-literal=CLAUDE_API_KEY=$CLAUDE_API_KEY \
  --from-literal=GEMINI_API_KEY=$GEMINI_API_KEY \
  --from-literal=Q_API_KEY=$Q_API_KEY \
  -n ralph-orchestrator
```

### 3. 永続ストレージ

```yaml
# k8s/03-pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ralph-workspace
  namespace: ralph-orchestrator
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: standard
  resources:
    requests:
      storage: 10Gi
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ralph-cache
  namespace: ralph-orchestrator
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: standard
  resources:
    requests:
      storage: 5Gi
```

### 4. Deployment

```yaml
# k8s/04-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ralph-orchestrator
  namespace: ralph-orchestrator
  labels:
    app: ralph-orchestrator
spec:
  replicas: 1
  selector:
    matchLabels:
      app: ralph-orchestrator
  template:
    metadata:
      labels:
        app: ralph-orchestrator
    spec:
      serviceAccountName: ralph-sa
      containers:
      - name: ralph
        image: ghcr.io/mikeyobrien/ralph-orchestrator:v1.0.0
        imagePullPolicy: Always
        envFrom:
        - configMapRef:
            name: ralph-config
        - secretRef:
            name: ralph-secrets
        resources:
          requests:
            memory: "2Gi"
            cpu: "1"
          limits:
            memory: "4Gi"
            cpu: "2"
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        - name: cache
          mountPath: /app/.cache
        - name: prompts
          mountPath: /prompts
        livenessProbe:
          exec:
            command:
            - python
            - -c
            - "import sys; sys.exit(0)"
          initialDelaySeconds: 30
          periodSeconds: 30
        readinessProbe:
          exec:
            command:
            - python
            - -c
            - "import os; sys.exit(0 if os.path.exists('/app/ralph_orchestrator.py') else 1)"
          initialDelaySeconds: 10
          periodSeconds: 10
      volumes:
      - name: workspace
        persistentVolumeClaim:
          claimName: ralph-workspace
      - name: cache
        persistentVolumeClaim:
          claimName: ralph-cache
      - name: prompts
        configMap:
          name: ralph-prompts
```

### 5. Service と監視

```yaml
# k8s/05-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: ralph-metrics
  namespace: ralph-orchestrator
  labels:
    app: ralph-orchestrator
spec:
  type: ClusterIP
  ports:
  - port: 8080
    targetPort: 8080
    name: metrics
  selector:
    app: ralph-orchestrator
---
# k8s/06-servicemonitor.yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: ralph-orchestrator
  namespace: ralph-orchestrator
spec:
  selector:
    matchLabels:
      app: ralph-orchestrator
  endpoints:
  - port: metrics
    interval: 30s
    path: /metrics
```

### 6. 一度きりのタスク用の Job

```yaml
# k8s/07-job.yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: ralph-task
  namespace: ralph-orchestrator
spec:
  backoffLimit: 3
  activeDeadlineSeconds: 14400
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: ralph
        image: ghcr.io/mikeyobrien/ralph-orchestrator:v1.0.0
        envFrom:
        - configMapRef:
            name: ralph-config
        - secretRef:
            name: ralph-secrets
        args:
        - "--agent=claude"
        - "--prompt=/prompts/task.md"
        - "--max-iterations=50"
        volumeMounts:
        - name: prompts
          mountPath: /prompts
        - name: output
          mountPath: /output
      volumes:
      - name: prompts
        configMap:
          name: ralph-prompts
      - name: output
        emptyDir: {}
```

## Helm チャートによるデプロイ

### Helm でインストールする

```bash
# リポジトリを追加する
helm repo add ralph https://mikeyobrien.github.io/ralph-orchestrator/charts
helm repo update

# カスタム値でインストールする
helm install ralph ralph/ralph-orchestrator \
  --namespace ralph-orchestrator \
  --create-namespace \
  --set apiKeys.claude=$CLAUDE_API_KEY \
  --set apiKeys.gemini=$GEMINI_API_KEY \
  --set config.maxIterations=100
```

### カスタム values.yaml

```yaml
# values.yaml
replicaCount: 1

image:
  repository: ghcr.io/mikeyobrien/ralph-orchestrator
  tag: v1.0.0
  pullPolicy: IfNotPresent

apiKeys:
  claude: ""
  gemini: ""
  q: ""

config:
  agent: "auto"
  maxIterations: 100
  maxRuntime: 14400
  checkpointInterval: 5
  verbose: true
  enableMetrics: true

resources:
  requests:
    memory: "2Gi"
    cpu: "1"
  limits:
    memory: "4Gi"
    cpu: "2"

persistence:
  enabled: true
  storageClass: "standard"
  workspace:
    size: 10Gi
  cache:
    size: 5Gi

autoscaling:
  enabled: false
  minReplicas: 1
  maxReplicas: 10
  targetCPUUtilizationPercentage: 80

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s

ingress:
  enabled: false
  className: "nginx"
  annotations: {}
  hosts:
    - host: ralph.example.com
      paths:
        - path: /
          pathType: Prefix
```

## 水平ポッドオートスケーリング

```yaml
# k8s/08-hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: ralph-hpa
  namespace: ralph-orchestrator
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: ralph-orchestrator
  minReplicas: 1
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

## スケジュールされたタスク用の CronJob

```yaml
# k8s/09-cronjob.yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: ralph-daily
  namespace: ralph-orchestrator
spec:
  schedule: "0 2 * * *"  # 毎日午前 2 時
  jobTemplate:
    spec:
      template:
        spec:
          restartPolicy: OnFailure
          containers:
          - name: ralph
            image: ghcr.io/mikeyobrien/ralph-orchestrator:v1.0.0
            envFrom:
            - configMapRef:
                name: ralph-config
            - secretRef:
                name: ralph-secrets
            args:
            - "--agent=auto"
            - "--prompt=/prompts/daily-task.md"
```

## サービスアカウントと RBAC

```yaml
# k8s/10-rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: ralph-sa
  namespace: ralph-orchestrator
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: ralph-role
  namespace: ralph-orchestrator
rules:
- apiGroups: [""]
  resources: ["configmaps", "secrets"]
  verbs: ["get", "list", "watch"]
- apiGroups: [""]
  resources: ["pods", "pods/log"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: ralph-rolebinding
  namespace: ralph-orchestrator
roleRef:
  apiVersion: rbac.authorization.k8s.io/v1
  kind: Role
  name: ralph-role
subjects:
- kind: ServiceAccount
  name: ralph-sa
  namespace: ralph-orchestrator
```

## ネットワークポリシー

```yaml
# k8s/11-networkpolicy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ralph-network-policy
  namespace: ralph-orchestrator
spec:
  podSelector:
    matchLabels:
      app: ralph-orchestrator
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: TCP
      port: 443  # API 呼び出し用の HTTPS
    - protocol: TCP
      port: 53   # DNS
    - protocol: UDP
      port: 53   # DNS
```

## Prometheus による監視

```yaml
# k8s/12-prometheus-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: monitoring
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
    scrape_configs:
    - job_name: 'ralph-orchestrator'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - ralph-orchestrator
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: ralph-orchestrator
```

## クラウドプロバイダ固有

### Google Kubernetes Engine（GKE）

```bash
# クラスタを作成する
gcloud container clusters create ralph-cluster \
  --zone us-central1-a \
  --num-nodes 3 \
  --machine-type n1-standard-2

# 認証情報を取得する
gcloud container clusters get-credentials ralph-cluster \
  --zone us-central1-a

# GCR 用のシークレットを作成する
kubectl create secret docker-registry gcr-json-key \
  --docker-server=gcr.io \
  --docker-username=_json_key \
  --docker-password="$(cat ~/key.json)" \
  -n ralph-orchestrator
```

### Amazon EKS

```bash
# クラスタを作成する
eksctl create cluster \
  --name ralph-cluster \
  --region us-west-2 \
  --nodegroup-name workers \
  --node-type t3.medium \
  --nodes 3

# kubeconfig を更新する
aws eks update-kubeconfig \
  --name ralph-cluster \
  --region us-west-2
```

### Azure AKS

```bash
# クラスタを作成する
az aks create \
  --resource-group ralph-rg \
  --name ralph-cluster \
  --node-count 3 \
  --node-vm-size Standard_DS2_v2

# 認証情報を取得する
az aks get-credentials \
  --resource-group ralph-rg \
  --name ralph-cluster
```

## ArgoCD による GitOps

```yaml
# k8s/argocd-app.yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: ralph-orchestrator
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/mikeyobrien/ralph-orchestrator
    targetRevision: HEAD
    path: k8s
  destination:
    server: https://kubernetes.default.svc
    namespace: ralph-orchestrator
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
```

## トラブルシューティング

### ポッドの状況を確認する

```bash
# ポッドを取得する
kubectl get pods -n ralph-orchestrator

# ポッドを詳細表示する
kubectl describe pod <pod-name> -n ralph-orchestrator

# ログを表示する
kubectl logs -f <pod-name> -n ralph-orchestrator

# ポッドに入る
kubectl exec -it <pod-name> -n ralph-orchestrator -- /bin/bash
```

### よくある問題

#### ImagePullBackOff

```bash
# イメージ取得シークレットを確認する
kubectl get secrets -n ralph-orchestrator

# 取得シークレットを作成する
kubectl create secret docker-registry regcred \
  --docker-server=ghcr.io \
  --docker-username=USERNAME \
  --docker-password=TOKEN \
  -n ralph-orchestrator
```

#### PVC がバインドされない

```bash
# PVC の状況を確認する
kubectl get pvc -n ralph-orchestrator

# 利用可能なストレージクラスを確認する
kubectl get storageclass

# 必要なら PV を作成する
kubectl apply -f persistent-volume.yaml
```

#### OOMKilled

```bash
# メモリ上限を増やす
kubectl set resources deployment ralph-orchestrator \
  --limits=memory=8Gi \
  -n ralph-orchestrator
```

## ベストプラクティス

1. Ralph のデプロイを分離するため**名前空間を使う**
2. 最小権限アクセスのため **RBAC を実装する**
3. **シークレット管理を使う**（Sealed Secrets、External Secrets）
4. リソースの枯渇を防ぐため**リソース上限を設定する**
5. Prometheus/Grafana で**監視を有効にする**
6. セキュリティのため**ネットワークポリシーを使う**
7. 自動回復のため**ヘルスチェックを実装する**
8. 宣言的なデプロイのため **GitOps を使う**
9. 永続ボリュームの**定期的なバックアップ**
10. 高可用性のため**ポッド中断バジェットを使う**

## 本番の考慮事項

- **高可用性**: 複数のアベイラビリティゾーンにわたってデプロイする
- **災害復旧**: 定期的なバックアップとクロスリージョンのレプリケーション
- **セキュリティ**: ポッドセキュリティポリシー、ネットワークポリシー、RBAC
- **可観測性**: ロギング（ELK）、メトリクス（Prometheus）、トレーシング（Jaeger）
- **コスト最適化**: スポットインスタンス、オートスケーリング、リソースクォータを使う
- **コンプライアンス**: 監査ロギング、保存時と転送時の暗号化

## 次のステップ

- [CI/CD 統合](ci-cd.ja.md) - Kubernetes デプロイの自動化
- [本番ガイド](production.ja.md) - 本番のベストプラクティス
- [監視のセットアップ](../advanced/monitoring.ja.md) - 完全な可観測性
