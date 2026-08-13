export type UploadStatus = 'PENDING' | 'RECEIVED' | 'VALIDATING' | 'PROCESSING' | 'COMPLETED' | 'FAILED';
export type VerificationStatus = 'Pending' | 'Completed';

export interface InformationDetails {
  informationFy: string;
  informationSourceType: string;
  informationSourceDescription: string;
  informationType: string;
  informationDescription: string;
  informationValue: string;
  source: string;
  finding: string;
}

export interface VerificationDetails {
  actionableAy: string;
  statutoryReason: string;
  verificationResultType: string;
  incomeEscapingAssessmentValue: string;
  informationValue: string;
  resultDescription: string;
}

export interface PersonRow {
  serialNo: string;
  caseId: string;
  pan: string;
  name: string;
  dobDoi: string;
  mobile: string;
  email: string;
  pinCode: string;
  address: string;
  state: string;
  verificationStatus: VerificationStatus;
  informationDetails: InformationDetails;
  verificationDetails: VerificationDetails;
}

export interface AttachedDocument {
  id: string;
  fileName: string;
  docType: string;
  description: string;
  attachedFor: 'Selected Rows' | 'All Rows';
  rowCaseIds: string[];
  file?: File;
}

export interface PacketDetails {
  reportType: string;
  referenceNumber: string;
  description: string;
  submittingPersonName: string;
  submittingPersonMobile: string;
  submittingPersonEmail: string;
  submittingPersonAddress: string;
}

export interface DraftState {
  version: 1;
  savedAt: string;
  step: number;
  packet: PacketDetails;
  rows: PersonRow[];
  documents: Array<Omit<AttachedDocument, 'file'>>;
}

export interface ApiError {
  code?: string;
  message?: string;
  fieldErrors?: Array<{ field: string; reason: string }>;
}

export interface PacketResponse extends PacketDetails {
  batchNumber: string;
  approvalStatus: 'PENDING' | 'APPROVED' | 'REJECTED';
  createdAt: string;
}

export interface CaseResponse extends Record<string, unknown> {
  caseId: string;
  status: string;
  approvalStatus: string;
  createdAt: string;
  updatedAt?: string;
}

export interface InitiateUploadResponse {
  uploadId: string;
  multipart: boolean;
  uploadUrl?: string | null;
  contentType?: string | null;
  minPartSizeBytes?: number | null;
  recommendedPartSizeBytes?: number | null;
  expiresAt: string;
}

export interface PresignPartResponse {
  partNumber: number;
  uploadUrl: string;
  expiresAt: string;
}

export interface UploadedPartSummary {
  partNumber: number;
  eTag: string;
  sizeBytes: number;
}
