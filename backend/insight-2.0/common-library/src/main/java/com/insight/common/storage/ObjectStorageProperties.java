package com.insight.common.storage;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "insight.storage")
public class ObjectStorageProperties {

    private String endpoint;
    private String region = "us-east-1";
    private String bucket;
    private String accessKey;
    private String secretKey;
    private boolean pathStyleAccess = true;

    /** Files at or above this size use multipart upload instead of a single presigned PUT. */
    private long multipartThresholdBytes = 100L * 1024 * 1024;

    /** Starting part size for the client's adaptive sizing algorithm (see docs/resumable-uploads.md). */
    private long defaultPartSizeBytes = 8L * 1024 * 1024;

    public String getEndpoint() { return endpoint; }
    public void setEndpoint(String endpoint) { this.endpoint = endpoint; }

    public String getRegion() { return region; }
    public void setRegion(String region) { this.region = region; }

    public String getBucket() { return bucket; }
    public void setBucket(String bucket) { this.bucket = bucket; }

    public String getAccessKey() { return accessKey; }
    public void setAccessKey(String accessKey) { this.accessKey = accessKey; }

    public String getSecretKey() { return secretKey; }
    public void setSecretKey(String secretKey) { this.secretKey = secretKey; }

    public boolean isPathStyleAccess() { return pathStyleAccess; }
    public void setPathStyleAccess(boolean pathStyleAccess) { this.pathStyleAccess = pathStyleAccess; }

    public long getMultipartThresholdBytes() { return multipartThresholdBytes; }
    public void setMultipartThresholdBytes(long multipartThresholdBytes) { this.multipartThresholdBytes = multipartThresholdBytes; }

    public long getDefaultPartSizeBytes() { return defaultPartSizeBytes; }
    public void setDefaultPartSizeBytes(long defaultPartSizeBytes) { this.defaultPartSizeBytes = defaultPartSizeBytes; }
}
