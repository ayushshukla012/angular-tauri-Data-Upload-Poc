package com.insight.upload.controller;

import com.insight.upload.dto.CaseDocumentResponse;
import com.insight.upload.dto.CaseResponse;
import com.insight.upload.dto.SubmitCaseRequest;
import com.insight.upload.service.CaseService;
import jakarta.validation.Valid;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/v1/cases")
public class CaseController {

    private final CaseService caseService;

    public CaseController(CaseService caseService) {
        this.caseService = caseService;
    }

    /**
     * Submits an ITD case's metadata — the direct-API replacement for the legacy XML-manifest
     * handoff. Idempotent: resubmitting the same caseId (the client's offline-reconnect retry)
     * returns the existing case unchanged rather than erroring — see CaseService.submitCase.
     */
    @PostMapping
    public ResponseEntity<CaseResponse> submitCase(@Valid @RequestBody SubmitCaseRequest request) {
        CaseResponse response = caseService.submitCase(request);
        return ResponseEntity.status(HttpStatus.CREATED).body(response);
    }

    @GetMapping("/{caseId}")
    public ResponseEntity<CaseResponse> getCase(@PathVariable String caseId) {
        return ResponseEntity.ok(caseService.getCase(caseId));
    }

    /** Documents attached to this case — each backed by a row in the generic uploads table. */
    @GetMapping("/{caseId}/documents")
    public ResponseEntity<List<CaseDocumentResponse>> listDocuments(@PathVariable String caseId) {
        return ResponseEntity.ok(caseService.listDocuments(caseId));
    }
}
