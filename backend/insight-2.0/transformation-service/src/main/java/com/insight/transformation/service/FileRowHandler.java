package com.insight.transformation.service;

import java.util.Map;

@FunctionalInterface
public interface FileRowHandler {

    void onRow(long rowNumber, Map<String, String> fields);
}
