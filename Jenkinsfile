/**
 * Jenkinsfile — agent-meter OSS (dnorio/agent-meter)
 *
 * Multibranch CI: fmt → clippy → test → build → smoke demo
 * Publishes GitHub commit status: jenkins/agent-meter
 * Runs on external CI K8s — zero GitHub Actions cost.
 */

pipeline {
  agent {
    kubernetes {
      yaml '''
apiVersion: v1
kind: Pod
metadata:
  labels:
    jenkins: agent-meter-oss
spec:
  containers:
  - name: rust
    image: rust:1.88-bookworm
    command: ["sleep"]
    args: ["infinity"]
    tty: true
    resources:
      requests:
        cpu: "1"
        memory: "2Gi"
      limits:
        cpu: "8"
        memory: "16Gi"
'''
    }
  }

  environment {
    CARGO_TERM_COLOR = 'always'
    CARGO_INCREMENTAL = '0'
    GITHUB_REPOSITORY = 'dnorio/agent-meter'
    GITHUB_STATUS_CONTEXT = 'jenkins/agent-meter'
    SONAR_HOST_URL = 'https://sonar.ci.example.com'
    SONAR_PROJECT_KEY = 'agent-meter'
    SONAR_TOKEN = credentials('sonar-token')
  }

  options {
    skipDefaultCheckout(true)
    timeout(time: 45, unit: 'MINUTES')
    disableConcurrentBuilds(abortPrevious: true)
    buildDiscarder(logRotator(numToKeepStr: '25'))
  }

  stages {
    stage('Prepare') {
      steps {
        container('rust') {
          checkout scm
          sh '''#!/usr/bin/env bash
set -euo pipefail
git config --global --add safe.directory "${WORKSPACE}"
export CODEQL_SHA="$(git rev-parse HEAD)"
echo "HEAD=${CODEQL_SHA}"
'''
          script {
            env.CODEQL_SHA = sh(returnStdout: true, script: 'git rev-parse HEAD').trim()
            env.CITOOLS_BRANCH = env.CHANGE_BRANCH ?: env.BRANCH_NAME ?: 'unknown'
          }
          withCredentials([usernamePassword(credentialsId: 'github-pat', usernameVariable: 'GIT_USER', passwordVariable: 'GITHUB_TOKEN')]) {
            sh '''#!/usr/bin/env bash
set -euo pipefail
export CODEQL_SHA="$(git rev-parse HEAD)"
export GITHUB_TOKEN="${GITHUB_TOKEN}"
export BUILD_URL="${BUILD_URL}"
bash scripts/ci/github-status.sh pending "agent-meter CI running" "${BUILD_URL}" || true
'''
          }
        }
      }
    }

    stage('Format') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
rustup component add rustfmt
cargo fmt --all -- --check
echo "✓ cargo fmt"
'''
        }
      }
    }

    stage('Clippy') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
rustup component add clippy
cargo clippy --workspace --all-targets
echo "✓ clippy"
'''
        }
      }
    }

    stage('Test') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
cargo test -p agent-meter-collector -p agent-meter-db
echo "✓ tests"
'''
        }
      }
    }

    stage('Build release') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
cargo build --workspace --release
ls -lh target/release/agent-meter-collector
echo "✓ release build"
'''
        }
      }
    }

    stage('Smoke demo') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
bash scripts/ci/smoke-demo.sh
'''
        }
      }
    }
  }

  post {
    success {
      echo "✓ agent-meter OSS CI PASSED"
    }
    failure {
      echo "✗ agent-meter OSS CI FAILED"
    }
    always {
      script {
        def state = currentBuild.currentResult == 'SUCCESS' ? 'success' : 'failure'
        def desc = "Build #${env.BUILD_NUMBER} ${currentBuild.currentResult}"
        withCredentials([usernamePassword(credentialsId: 'github-pat', usernameVariable: 'GIT_USER', passwordVariable: 'GITHUB_TOKEN')]) {
          sh """#!/usr/bin/env bash
set -euo pipefail
export CODEQL_SHA="${env.CODEQL_SHA}"
export GITHUB_TOKEN="\${GITHUB_TOKEN}"
export BUILD_URL="${env.BUILD_URL}"
bash scripts/ci/github-status.sh ${state} "${desc}" "${env.BUILD_URL}" || true
"""
        }
      }
    }
  }
}
