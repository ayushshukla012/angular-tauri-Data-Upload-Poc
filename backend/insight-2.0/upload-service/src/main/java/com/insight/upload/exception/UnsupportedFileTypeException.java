package com.insight.upload.exception;

import com.insight.common.exception.BaseException;

/**
 * A deterministic client-input rejection: the file's extension isn't one this service accepts.
 * Retrying the identical request will never succeed, so this must map to 400 — not bubble up as
 * an unhandled 500, which previously caused clients to burn their retry budget on a request that
 * could never work (see insight-javafs's CasesController.startDocumentUploadWithRetry).
 */
public class UnsupportedFileTypeException extends BaseException {

    public UnsupportedFileTypeException(String fileName) {
        super("UNSUPPORTED_FILE_TYPE", "Unsupported file type: " + fileName);
    }
}
