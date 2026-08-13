# Build context must be the repo root (not this directory) — Maven needs the parent pom
# and sibling modules (common-library, protos) present to resolve this service's dependencies.
# docker build -f transformation-service/Dockerfile -t <tag> .

FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /workspace
COPY pom.xml .
COPY common-library ./common-library
COPY protos ./protos
COPY transformation-service ./transformation-service
RUN mvn -B -ntp -pl transformation-service -am install -DskipTests

FROM eclipse-temurin:21-jre-alpine
WORKDIR /app
COPY --from=build /workspace/transformation-service/target/transformation-service-*.jar app.jar
EXPOSE 8082 9091
ENTRYPOINT ["java", "-jar", "app.jar"]
