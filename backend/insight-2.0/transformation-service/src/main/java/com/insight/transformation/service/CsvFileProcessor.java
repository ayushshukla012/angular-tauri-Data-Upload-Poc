package com.insight.transformation.service;

import org.apache.commons.csv.CSVFormat;
import org.apache.commons.csv.CSVParser;
import org.apache.commons.csv.CSVRecord;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

@Component
public class CsvFileProcessor implements FileProcessorStrategy {

    @Override
    public String fileType() {
        return "CSV";
    }

    @Override
    public void process(InputStream inputStream, FileRowHandler rowHandler) throws IOException {
        CSVFormat format = CSVFormat.DEFAULT.builder()
                .setHeader()
                .setSkipHeaderRecord(true)
                .setTrim(true)
                .build();

        try (CSVParser parser = CSVParser.parse(new InputStreamReader(inputStream, StandardCharsets.UTF_8), format)) {
            long rowNumber = 0;
            for (CSVRecord record : parser) {
                rowNumber++;
                Map<String, String> fields = new LinkedHashMap<>();
                record.toMap().forEach(fields::put);
                rowHandler.onRow(rowNumber, fields);
            }
        }
    }
}
