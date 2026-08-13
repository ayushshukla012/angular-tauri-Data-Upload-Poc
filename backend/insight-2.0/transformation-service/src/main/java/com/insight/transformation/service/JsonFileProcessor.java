package com.insight.transformation.service;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.io.InputStream;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Expects a top-level JSON array of flat objects. Uses Jackson's streaming API so a
 * multi-million-record file is never materialized in memory as a single tree.
 */
@Component
public class JsonFileProcessor implements FileProcessorStrategy {

    private final ObjectMapper objectMapper;

    public JsonFileProcessor(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    @Override
    public String fileType() {
        return "JSON";
    }

    @Override
    public void process(InputStream inputStream, FileRowHandler rowHandler) throws IOException {
        try (JsonParser parser = objectMapper.getFactory().createParser(inputStream)) {
            if (parser.nextToken() != JsonToken.START_ARRAY) {
                throw new IOException("Expected a top-level JSON array of records");
            }

            long rowNumber = 0;
            while (parser.nextToken() == JsonToken.START_OBJECT) {
                rowNumber++;
                Map<String, Object> node = objectMapper.readValue(parser, new TypeReference<Map<String, Object>>() {});
                Map<String, String> fields = new LinkedHashMap<>();
                node.forEach((key, value) -> fields.put(key, value == null ? null : value.toString()));
                rowHandler.onRow(rowNumber, fields);
            }
        }
    }
}
