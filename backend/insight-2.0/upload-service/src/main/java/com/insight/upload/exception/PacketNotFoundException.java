package com.insight.upload.exception;

import com.insight.common.exception.ResourceNotFoundException;

public class PacketNotFoundException extends ResourceNotFoundException {

    public PacketNotFoundException(String batchNumber) {
        super("Packet not found: " + batchNumber);
    }
}
