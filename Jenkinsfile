node {
    def bin
    def app
    
    stage('Clone repository') {
        checkout scm
        sh 'git submodule update --init --recursive'
    }

    stage('Build app') {
        sh 'cp -f docker/Dockerfile.build Dockerfile'
        bin = docker.build("storiqateam/saga-interm:${env.BRANCH_NAME}")
        sh 'rm -f Dockerfile'
    }
    
    stage('Get binary') {
        sh "docker run -i --rm -v ${env.PWD}:/mnt/ -e 'cp -f /app/target/release/saga_coordinator_runner /mnt' storiqateam/saga-interm:${env.BRANCH_NAME}"
    }
    
    stage('')

//     stage('Push image') {
//         docker.withRegistry('https://registry.hub.docker.com', 'docker-hub-credentials') {
//             app.push("${env.BUILD_NUMBER}")
//             app.push("latest")
//         }
//     }
}
