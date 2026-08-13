package com.insight.transformation.service;

import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.stream.Collectors;

@Component
public class FileProcessorRegistry {

    private final Map<String, FileProcessorStrategy> strategiesByFileType;

    public FileProcessorRegistry(List<FileProcessorStrategy> strategies) {
        this.strategiesByFileType = strategies.stream()
                .collect(Collectors.toUnmodifiableMap(FileProcessorStrategy::fileType, Function.identity()));
    }

    public FileProcessorStrategy resolve(String fileType) {
        FileProcessorStrategy strategy = strategiesByFileType.get(fileType);
        if (strategy == null) {
            throw new IllegalArgumentException("No file processor registered for file type: " + fileType);
        }
        return strategy;
    }
}
