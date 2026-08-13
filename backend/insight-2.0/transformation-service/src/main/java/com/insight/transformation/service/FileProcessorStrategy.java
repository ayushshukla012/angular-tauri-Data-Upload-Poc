package com.insight.transformation.service;

import java.io.IOException;
import java.io.InputStream;

/**
 * One implementation per onboarded file type. Adding a new file type means adding a new
 * {@code @Component} here and registering its key in {@link FileProcessorRegistry} — no
 * changes to the gRPC entry point, the worker, or any other file type's processor.
 */
public interface FileProcessorStrategy {

    String fileType();

    void process(InputStream inputStream, FileRowHandler rowHandler) throws IOException;
}
