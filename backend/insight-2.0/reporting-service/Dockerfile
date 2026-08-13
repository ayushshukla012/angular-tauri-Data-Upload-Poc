# Build context must be the repo root (not this directory) — Maven needs the parent pom
# and sibling modules (common-library, protos) present to resolve this service's dependencies.
# docker build -f reporting-service/Dockerfile -t <tag> .

FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /workspace
COPY pom.xml .
COPY common-library ./common-library
COPY protos ./protos
COPY reporting-service ./reporting-service
RUN mvn -B -ntp -pl reporting-service -am install -DskipTests

FROM eclipse-temurin:21-jre-alpine
WORKDIR /app
COPY --from=build /workspace/reporting-service/target/reporting-service-*.jar app.jar
EXPOSE 8085 9093
ENTRYPOINT ["java", "-jar", "app.jar"]
