package com.insight.transformation;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.scheduling.annotation.EnableScheduling;

@SpringBootApplication
@EnableScheduling
public class TransformationServiceApplication {

    public static void main(String[] args) {
        SpringApplication.run(TransformationServiceApplication.class, args);
    }
}
