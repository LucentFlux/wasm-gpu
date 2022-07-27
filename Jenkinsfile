pipeline {
  agent any
  stages {
    environment {
      FULL_TESTS = 'true'
    }

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
        sh 'cargo test --no-fail-fast --package wasm-spirv --test run -- --test-threads 64 > ./test_results.txt'
        sh 'python gen_test_report.py'
        junit 'test_results.xml'
      }
    }
  }
}