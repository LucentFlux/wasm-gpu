pipeline {
  agent any
  stages {
    stage('Build') {
      steps {
        sh 'cargo build --package wasm-spirv'
      }
    }

    stage('Test') {
      steps {
        sh 'cargo test --package wasm-spirv'
      }
    }

  }
}