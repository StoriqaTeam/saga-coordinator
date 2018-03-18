node {
    def bin
    def app

    stage('Build app') {
        bin = docker.build("storiqateam/saga${env.BRANCH_NAME}","docker/Dockerfile.build")
    }
    
    stage('Get binary') {
        bin.inside("cp -f /app/target/release/saga_coordinator_runner .")
    }

//     stage('Push image') {
//         docker.withRegistry('https://registry.hub.docker.com', 'docker-hub-credentials') {
//             app.push("${env.BUILD_NUMBER}")
//             app.push("latest")
//         }
//     }
}
