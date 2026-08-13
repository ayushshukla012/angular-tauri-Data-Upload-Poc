package com.insight.upload.service;

import com.insight.common.dto.ApiError;
import com.insight.upload.dto.SubmitCaseRequest;
import org.springframework.stereotype.Component;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/**
 * Field-format rules only — bean validation (@NotBlank on the DTO) already rejects blank fields
 * before this runs. Deliberately does NOT reject a case id that already exists: unlike a CSV
 * import batch (a client-side data-entry concern), a resubmission of the same case id here is
 * the client's offline-reconnect retry (see insight-javafs's UploadQueue.maybeAutoResubmitCases)
 * and must be idempotent, not an error — see CaseService.submitCase and proposed_plan.md §5.5.
 */
@Component
public class CaseValidator {
    private static final Pattern PAN_PATTERN = Pattern.compile("[A-Z]{5}[0-9]{4}[A-Z]");
    private static final Pattern PHONE_PATTERN = Pattern.compile("[0-9]{10}");

    public List<ApiError.FieldError> validate(SubmitCaseRequest request) {
        List<ApiError.FieldError> errors = new ArrayList<>();

        if (request.sourcePan() != null && !PAN_PATTERN.matcher(request.sourcePan()).matches()) {
            errors.add(new ApiError.FieldError("sourcePan", "Must match format AAAAA9999A"));
        }
        if (request.mobileNumber() != null && !PHONE_PATTERN.matcher(request.mobileNumber()).matches()) {
            errors.add(new ApiError.FieldError("mobileNumber", "Must be exactly 10 digits"));
        }

        return errors;
    }
}
