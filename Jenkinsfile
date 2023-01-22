pipeline {
  agent any
  stages {
    stage('Pull Submodules') {
      steps {
        sh 'git submodule update --init --recursive'
      }
    }

    stage('Build') {
      steps {
        sh 'cargo build --package wasm-spirv --verbose'
      }
    }

    stage('Test') {
      steps {
        sh 'cargo install cargo2junit'
        sh 'cargo test --package wasm-spirv --no-fail-fast -- -Z unstable-options --format json --test-threads 48 --report-time | cargo2junit > results.xml || true'
        archiveArtifacts '**/results.xml'
        junit 'results.xml'
      }
    }

  }
  environment {
    FULL_TESTS = 'true'
  }
}