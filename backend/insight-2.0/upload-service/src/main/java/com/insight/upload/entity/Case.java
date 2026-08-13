package com.insight.upload.entity;

import jakarta.persistence.*;
import java.time.Instant;

/**
 * A Verification Report (VSN) — mirrors the legacy "Verification Report Upload Utility"'s
 * Verification Result screen field-for-field (minus its XML/packaging step, which this platform
 * replaces with direct API submission). {@code id} is the VSN.
 */
@Entity
@Table(name = "cases")
public class Case {

    /** The natural case identifier from the source system (e.g. "CASE-1001") — the VSN, not a generated UUID. */
    @Id
    private String id;

    @Column(nullable = false)
    private String sourcePan;

    @Column(nullable = false)
    private String name;

    @Column(nullable = false)
    private String mobileNumber;

    @Column(nullable = false)
    private String designation;

    private String referenceNumber;
    private String dateOfBirth;
    @Column(columnDefinition = "TEXT")
    private String address;
    private String stateUtCode;
    private String pincode;
    private String email;

    private String informationFy;
    private String informationSourceType;
    @Column(columnDefinition = "TEXT")
    private String informationSourceDescription;
    private String informationType;
    @Column(columnDefinition = "TEXT")
    private String informationDescription;
    private String informationValue;

    private String natureOfVerification;
    private String actionableAy;

    @Column(name = "verification_result_type_1")
    private String verificationResultType1;
    @Column(name = "verification_result_description_1", columnDefinition = "TEXT")
    private String verificationResultDescription1;
    @Column(name = "verification_result_value_1")
    private String verificationResultValue1;
    @Column(name = "verification_result_type_2")
    private String verificationResultType2;
    @Column(name = "verification_result_description_2", columnDefinition = "TEXT")
    private String verificationResultDescription2;
    @Column(name = "verification_result_value_2")
    private String verificationResultValue2;
    @Column(name = "verification_result_type_3")
    private String verificationResultType3;
    @Column(name = "verification_result_description_3", columnDefinition = "TEXT")
    private String verificationResultDescription3;
    @Column(name = "verification_result_value_3")
    private String verificationResultValue3;

    @Column(columnDefinition = "TEXT")
    private String remarks;

    /** The Packet/Batch (see {@link Packet}) this case was submitted under, if any. */
    private String batchNumber;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private CaseStatus status;

    /** Approval-workflow state — separate from {@link #status}, which tracks upload/processing state. */
    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private ApprovalStatus approvalStatus = ApprovalStatus.PENDING;

    private String errorMessage;

    /** JSON-encoded map of any extra fields the source CSV/form included beyond the mandatory ones. */
    @Column(columnDefinition = "TEXT")
    private String extraFields;

    @Column(nullable = false)
    private Instant createdAt;

    private Instant updatedAt;

    protected Case() {
    }

    public Case(String id, String sourcePan, String name, String mobileNumber, String designation, String extraFields) {
        this.id = id;
        this.sourcePan = sourcePan;
        this.name = name;
        this.mobileNumber = mobileNumber;
        this.designation = designation;
        this.extraFields = extraFields;
        this.status = CaseStatus.RECEIVED;
        this.createdAt = Instant.now();
    }

    public void markStatus(CaseStatus status) {
        this.status = status;
        this.updatedAt = Instant.now();
    }

    public void markFailed(String errorMessage) {
        this.status = CaseStatus.FAILED;
        this.errorMessage = errorMessage;
        this.updatedAt = Instant.now();
    }

    public String getId() { return id; }
    public String getSourcePan() { return sourcePan; }
    public String getName() { return name; }
    public String getMobileNumber() { return mobileNumber; }
    public String getDesignation() { return designation; }
    public CaseStatus getStatus() { return status; }
    public ApprovalStatus getApprovalStatus() { return approvalStatus; }
    public void setApprovalStatus(ApprovalStatus approvalStatus) { this.approvalStatus = approvalStatus; }
    public String getErrorMessage() { return errorMessage; }
    public String getExtraFields() { return extraFields; }
    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }

    public String getReferenceNumber() { return referenceNumber; }
    public void setReferenceNumber(String referenceNumber) { this.referenceNumber = referenceNumber; }

    public String getDateOfBirth() { return dateOfBirth; }
    public void setDateOfBirth(String dateOfBirth) { this.dateOfBirth = dateOfBirth; }

    public String getAddress() { return address; }
    public void setAddress(String address) { this.address = address; }

    public String getStateUtCode() { return stateUtCode; }
    public void setStateUtCode(String stateUtCode) { this.stateUtCode = stateUtCode; }

    public String getPincode() { return pincode; }
    public void setPincode(String pincode) { this.pincode = pincode; }

    public String getEmail() { return email; }
    public void setEmail(String email) { this.email = email; }

    public String getInformationFy() { return informationFy; }
    public void setInformationFy(String informationFy) { this.informationFy = informationFy; }

    public String getInformationSourceType() { return informationSourceType; }
    public void setInformationSourceType(String informationSourceType) { this.informationSourceType = informationSourceType; }

    public String getInformationSourceDescription() { return informationSourceDescription; }
    public void setInformationSourceDescription(String informationSourceDescription) { this.informationSourceDescription = informationSourceDescription; }

    public String getInformationType() { return informationType; }
    public void setInformationType(String informationType) { this.informationType = informationType; }

    public String getInformationDescription() { return informationDescription; }
    public void setInformationDescription(String informationDescription) { this.informationDescription = informationDescription; }

    public String getInformationValue() { return informationValue; }
    public void setInformationValue(String informationValue) { this.informationValue = informationValue; }

    public String getNatureOfVerification() { return natureOfVerification; }
    public void setNatureOfVerification(String natureOfVerification) { this.natureOfVerification = natureOfVerification; }

    public String getActionableAy() { return actionableAy; }
    public void setActionableAy(String actionableAy) { this.actionableAy = actionableAy; }

    public String getVerificationResultType1() { return verificationResultType1; }
    public void setVerificationResultType1(String verificationResultType1) { this.verificationResultType1 = verificationResultType1; }

    public String getVerificationResultDescription1() { return verificationResultDescription1; }
    public void setVerificationResultDescription1(String verificationResultDescription1) { this.verificationResultDescription1 = verificationResultDescription1; }

    public String getVerificationResultValue1() { return verificationResultValue1; }
    public void setVerificationResultValue1(String verificationResultValue1) { this.verificationResultValue1 = verificationResultValue1; }

    public String getVerificationResultType2() { return verificationResultType2; }
    public void setVerificationResultType2(String verificationResultType2) { this.verificationResultType2 = verificationResultType2; }

    public String getVerificationResultDescription2() { return verificationResultDescription2; }
    public void setVerificationResultDescription2(String verificationResultDescription2) { this.verificationResultDescription2 = verificationResultDescription2; }

    public String getVerificationResultValue2() { return verificationResultValue2; }
    public void setVerificationResultValue2(String verificationResultValue2) { this.verificationResultValue2 = verificationResultValue2; }

    public String getVerificationResultType3() { return verificationResultType3; }
    public void setVerificationResultType3(String verificationResultType3) { this.verificationResultType3 = verificationResultType3; }

    public String getVerificationResultDescription3() { return verificationResultDescription3; }
    public void setVerificationResultDescription3(String verificationResultDescription3) { this.verificationResultDescription3 = verificationResultDescription3; }

    public String getVerificationResultValue3() { return verificationResultValue3; }
    public void setVerificationResultValue3(String verificationResultValue3) { this.verificationResultValue3 = verificationResultValue3; }

    public String getRemarks() { return remarks; }
    public void setRemarks(String remarks) { this.remarks = remarks; }

    public String getBatchNumber() { return batchNumber; }
    public void setBatchNumber(String batchNumber) { this.batchNumber = batchNumber; }
}
