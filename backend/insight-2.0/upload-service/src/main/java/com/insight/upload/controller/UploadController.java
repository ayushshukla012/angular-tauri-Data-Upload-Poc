package com.insight.upload.controller;

import com.insight.upload.dto.InitiateUploadRequest;
import com.insight.upload.dto.InitiateUploadResponse;
import com.insight.upload.dto.PresignPartResponse;
import com.insight.upload.dto.UploadPartsResponse;
import com.insight.upload.dto.UploadResponse;
import com.insight.upload.service.UploadService;
import jakarta.validation.Valid;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/v1/uploads")
public class UploadController {

    private final UploadService uploadService;

    public UploadController(UploadService uploadService) {
        this.uploadService = uploadService;
    }

    /**
     * Step 1: get a presigned URL (small files) or start a multipart upload (large files —
     * see docs/resumable-uploads.md). The client PUTs its file bytes directly to storage, not here.
     */
    @PostMapping("/initiate")
    public ResponseEntity<InitiateUploadResponse> initiate(@Valid @RequestBody InitiateUploadRequest request) {
        InitiateUploadResponse response = uploadService.initiate(request);
        return ResponseEntity.status(HttpStatus.CREATED).body(response);
    }

    /** Multipart only: a presigned URL for exactly one part, requested on demand. */
    @PostMapping("/{uploadId}/parts/{partNumber}/presign")
    public ResponseEntity<PresignPartResponse> presignPart(@PathVariable String uploadId, @PathVariable int partNumber) {
        return ResponseEntity.ok(uploadService.presignPart(uploadId, partNumber));
    }

    /** Multipart only: resume entry point — which parts are already uploaded. */
    @GetMapping("/{uploadId}/parts")
    public ResponseEntity<UploadPartsResponse> listParts(@PathVariable String uploadId) {
        return ResponseEntity.ok(uploadService.listUploadedParts(uploadId));
    }

    /** Step 2: call this once the upload succeeds (single PUT, or every part) — this is what starts the saga. */
    @PostMapping("/{uploadId}/complete")
    public ResponseEntity<UploadResponse> complete(@PathVariable String uploadId) {
        return ResponseEntity.ok(uploadService.complete(uploadId));
    }

    @GetMapping("/{uploadId}")
    public ResponseEntity<UploadResponse> getStatus(@PathVariable String uploadId) {
        return ResponseEntity.ok(uploadService.getStatus(uploadId));
    }
}
