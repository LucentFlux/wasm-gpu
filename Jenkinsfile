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
        sh 'cargo build --package wasm-spirv'
      }
    }

    stage('Test') {
      steps {
        sh 'cargo test --no-fail-fast --package wasm-spirv -- --test-threads 16'
      }
    }

  }
}