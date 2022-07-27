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
        sh 'cargo test --no-fail-fast --package wasm-spirv --test run > ./test_results.txt || true'
        sh 'python3 gen_test_report.py'
        junit 'test_results.xml'
      }
    }

  }
  environment {
    FULL_TESTS = 'false'
  }
}