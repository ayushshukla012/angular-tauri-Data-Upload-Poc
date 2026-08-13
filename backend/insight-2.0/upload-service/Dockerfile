# Build context must be the repo root (not this directory) — Maven needs the parent pom
# and sibling modules (common-library, protos) present to resolve this service's dependencies.
# docker build -f upload-service/Dockerfile -t <tag> .

FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /workspace
COPY pom.xml .
COPY common-library ./common-library
COPY protos ./protos
COPY upload-service ./upload-service
RUN mvn -B -ntp -pl upload-service -am install -DskipTests

FROM eclipse-temurin:21-jre-alpine
WORKDIR /app
COPY --from=build /workspace/upload-service/target/upload-service-*.jar app.jar
EXPOSE 8081 9095
ENTRYPOINT ["java", "-jar", "app.jar"]
