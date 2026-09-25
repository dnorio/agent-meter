/**
 * Jenkinsfile — agent-meter OSS (dnorio/agent-meter)
 *
 * Branches/PRs: fmt → clippy → test → build → smoke (jenkins/agent-meter)
 * Tags v*: release-build → gh release upload (T-365 Fase 3)
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
    image: rust:1.97-bookworm
    command: ["sleep"]
    args: ["infinity"]
    tty: true
    resources:
      requests:
        cpu: "2"
        memory: "4Gi"
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
    INSTALL_MINGW = '1'
    SONAR_HOST_URL = 'https://sonar.ssdnodes.dnor.io'
    SONAR_PROJECT_KEY = 'agent-meter-oss'
  }

  options {
    skipDefaultCheckout(true)
    timeout(time: 90, unit: 'MINUTES')
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
chmod +x scripts/ci/*.sh 2>/dev/null || true
export CODEQL_SHA="$(git rev-parse HEAD)"
echo "HEAD=${CODEQL_SHA} TAG=${TAG_NAME:-none}"
'''
          script {
            env.CODEQL_SHA = sh(returnStdout: true, script: 'git rev-parse HEAD').trim()
            env.CITOOLS_BRANCH = env.CHANGE_BRANCH ?: env.BRANCH_NAME ?: env.TAG_NAME ?: 'unknown'
          }
        }
      }
    }

    stage('GitHub status pending') {
      // Credentials never share a pod with untrusted PR cargo (CodeQL).
      // Status updates run on branch pushes only; GHA covers PR checks.
      when {
        allOf {
          not { buildingTag() }
          not { changeRequest() }
        }
      }
      steps {
        container('rust') {
          withCredentials([usernamePassword(credentialsId: 'github-pat', usernameVariable: 'GIT_USER', passwordVariable: 'GITHUB_TOKEN')]) {
            sh '''#!/usr/bin/env bash
set -euo pipefail
SHA="$(git rev-parse HEAD)"
REPO="${GITHUB_REPOSITORY:-dnorio/agent-meter}"
CONTEXT="${GITHUB_STATUS_CONTEXT:-jenkins/agent-meter}"
DESC="agent-meter CI running"
payload=$(printf '{"state":"pending","context":"%s","description":"%s","target_url":"%s"}' \
  "$CONTEXT" "$DESC" "${BUILD_URL:-}")
curl -sS -o /tmp/gh-status.json -w '%{http_code}' \
  -X POST \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/statuses/${SHA}" \
  -d "$payload" >/tmp/gh-status.code || true
echo "[github-status] pending → ${CONTEXT} (${SHA:0:8}) HTTP $(cat /tmp/gh-status.code 2>/dev/null || echo ?)"
'''
          }
        }
      }
    }

    stage('CI') {
      when {
        not { buildingTag() }
      }
      stages {
    stage('Format') {
      steps {
        container('rust') {
          sh '''#!/usr/bin/env bash
set -euo pipefail
bash scripts/ci/oss-scrub-check.sh
bash scripts/ci/secret-scan-history.sh
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
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}"
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

        stage('Capture e2e') {
          // Fixture + proxy-shaped OTLP contracts — IDE GUIs stay off-cluster.
          steps {
            container('rust') {
              sh '''#!/usr/bin/env bash
set -euo pipefail
chmod +x scripts/ci/capture-e2e.sh scripts/ci/capture-proxy-e2e.sh
bash scripts/ci/capture-e2e.sh
bash scripts/ci/capture-proxy-e2e.sh
cargo test -p agent-meter-collector --test otlp_regression -- --test-threads=1
echo "✓ capture e2e + proxy + otlp_regression"
'''
            }
          }
        }

        stage('Coverage') {
          // LCOV for Sonar — trusted branches only (heavy llvm-cov).
          when {
            not { changeRequest() }
          }
          steps {
            container('rust') {
              sh '''#!/usr/bin/env bash
set -euo pipefail
chmod +x scripts/ci/coverage-sonar.sh
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}"
export COVERAGE_MIN_LINES="${COVERAGE_MIN_LINES:-0}"
bash scripts/ci/coverage-sonar.sh
echo "✓ coverage LCOV"
'''
            }
          }
        }

        stage('SonarQube') {
          // Sonar token only on trusted branch builds — never after PR cargo (CodeQL).
          when {
            not { changeRequest() }
          }
          steps {
            container('rust') {
              withCredentials([string(credentialsId: 'sonar-token', variable: 'SONAR_TOKEN')]) {
                sh '''#!/usr/bin/env bash
set -euo pipefail
if [ -z "${SONAR_TOKEN:-}" ]; then
  echo "SONAR_TOKEN unset — skip"
  exit 0
fi
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends openjdk-17-jre-headless ca-certificates unzip curl python3
if ! command -v sonar-scanner >/dev/null 2>&1; then
  SONAR_VERSION="7.1.0.4889"
  # Pin from binaries.sonarsource.com …linux-x64.zip.sha256
  SONAR_SHA256="b4d2a001d65b489f9effe1ea8a78495db1b152f124d7f7b058aad8651c7e1484"
  curl -sSL "https://binaries.sonarsource.com/Distribution/sonar-scanner-cli/sonar-scanner-cli-${SONAR_VERSION}-linux-x64.zip" -o /tmp/sonar.zip
  echo "${SONAR_SHA256}  /tmp/sonar.zip" | sha256sum -c -
  unzip -q /tmp/sonar.zip -d /opt
  ln -sf /opt/sonar-scanner-*/bin/sonar-scanner /usr/local/bin/sonar-scanner
fi
curl -sS -H "Authorization: Bearer ${SONAR_TOKEN}" -X POST \
  "${SONAR_HOST_URL}/api/projects/create?project=${SONAR_PROJECT_KEY}&name=agent-meter-oss" >/dev/null 2>&1 || true
# Bind Sonar way QG (idempotent)
curl -sS -H "Authorization: Bearer ${SONAR_TOKEN}" -X POST \
  "${SONAR_HOST_URL}/api/qualitygates/select?projectKey=${SONAR_PROJECT_KEY}&gateName=Sonar%20way" \
  >/dev/null 2>&1 || true
ARGS=(
  -Dsonar.projectKey="${SONAR_PROJECT_KEY}"
  -Dsonar.projectName=agent-meter-oss
  -Dsonar.host.url="${SONAR_HOST_URL}"
  -Dsonar.sources=crates
  -Dsonar.exclusions="**/target/**,**/ui/**,**/migrations/**"
  -Dsonar.coverage.exclusions="**/ui/**,**/*.js,**/*.css,**/main.rs,**/bin/**,**/postgres.rs,**/demo.rs,**/telemetry.rs,**/collector/src/lib.rs,**/collector/src/db.rs"
  -Dsonar.tests=crates
  -Dsonar.test.inclusions="**/*_test.rs,**/tests/**"
  -Dsonar.sourceEncoding=UTF-8
  -Dsonar.scm.revision="$(git rev-parse HEAD)"
)
if [[ -f target/coverage/lcov.info ]]; then
  ARGS+=(-Dsonar.rust.lcov.reportPaths=target/coverage/lcov.info)
fi
# Scanner reads SONAR_TOKEN from environment (no -Dsonar.token=).
sonar-scanner "${ARGS[@]}"
echo "✓ Sonar submitted → ${SONAR_HOST_URL}/dashboard?id=${SONAR_PROJECT_KEY}"
'''
              }
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
    }

    stage('Release') {
      when {
        allOf {
          buildingTag()
          expression { env.TAG_NAME ==~ /^v\d+\.\d+.*/ }
        }
      }
      stages {
        stage('Build artifacts') {
          steps {
            container('rust') {
              sh '''#!/usr/bin/env bash
set -euo pipefail
export TAG_NAME="${TAG_NAME}"
bash scripts/ci/release-build.sh
'''
            }
          }
        }

        stage('Publish GitHub Release') {
          steps {
            container('rust') {
              withCredentials([usernamePassword(credentialsId: 'github-pat', usernameVariable: 'GIT_USER', passwordVariable: 'GITHUB_TOKEN')]) {
                sh '''#!/usr/bin/env bash
set -euo pipefail
export GITHUB_TOKEN="${GITHUB_TOKEN}"
export TAG_NAME="${TAG_NAME}"
bash scripts/ci/release-publish.sh "${TAG_NAME}"
'''
              }
            }
          }
        }
      }
    }
  }

  post {
    success {
      echo "✓ agent-meter OSS ${env.TAG_NAME ? 'release' : 'CI'} PASSED"
    }
    failure {
      echo "✗ agent-meter OSS FAILED"
    }
    always {
      script {
        // No github-pat after untrusted PR cargo (CodeQL). Branch builds only.
        if (env.TAG_NAME || env.CHANGE_ID) {
          return
        }
        def state = currentBuild.currentResult == 'SUCCESS' ? 'success' : 'failure'
        def desc = "Build #${env.BUILD_NUMBER} ${currentBuild.currentResult}".take(140)
        withEnv(["STATUS_STATE=${state}", "STATUS_DESC=${desc}"]) {
          withCredentials([usernamePassword(credentialsId: 'github-pat', usernameVariable: 'GIT_USER', passwordVariable: 'GITHUB_TOKEN')]) {
            sh '''#!/usr/bin/env bash
set -euo pipefail
SHA="${CODEQL_SHA:-$(git rev-parse HEAD 2>/dev/null || true)}"
REPO="${GITHUB_REPOSITORY:-dnorio/agent-meter}"
CONTEXT="${GITHUB_STATUS_CONTEXT:-jenkins/agent-meter}"
DESC=$(printf '%s' "${STATUS_DESC}" | head -c 140 | sed 's/\\\\/\\\\\\\\/g; s/"/\\\\"/g')
payload=$(printf '{"state":"%s","context":"%s","description":"%s","target_url":"%s"}' \
  "${STATUS_STATE}" "$CONTEXT" "$DESC" "${BUILD_URL:-}")
code=$(curl -sS -o /tmp/gh-status.json -w '%{http_code}' \
  -X POST \
  -H "Authorization: Bearer ${GITHUB_TOKEN}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/statuses/${SHA}" \
  -d "$payload" || echo 000)
echo "[github-status] ${STATUS_STATE} → ${CONTEXT} (${SHA:0:8}) HTTP ${code}"
'''
          }
        }
      }
    }
  }
}
