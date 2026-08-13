package com.insight.common.storage;

import java.io.InputStream;
import java.net.URI;
import java.time.Duration;
import java.util.List;

public interface ObjectStorageClient {

    void put(String key, InputStream data, long contentLength, String contentType);

    InputStream get(String key);

    /**
     * A time-limited URL the caller can PUT bytes to directly — the file never has to pass
     * through this service. {@code ttl} bounds how long the URL stays valid.
     */
    URI presignPut(String key, String contentType, Duration ttl);

    boolean exists(String key);

    /** Starts a multipart upload, returning the store's own upload ID (see docs/resumable-uploads.md). */
    String createMultipartUpload(String key, String contentType);

    /** A time-limited URL the caller can PUT exactly one part's bytes to, generated on demand. */
    URI presignUploadPart(String key, String multipartUploadId, int partNumber, Duration ttl);

    /** The store's own record of which parts exist for this multipart upload — the source of truth for resume. */
    List<PartInfo> listParts(String key, String multipartUploadId);

    /** Assembles all uploaded parts into the final object. */
    void completeMultipartUpload(String key, String multipartUploadId, List<PartInfo> parts);

    /** Cleans up an abandoned multipart upload. */
    void abortMultipartUpload(String key, String multipartUploadId);
}
