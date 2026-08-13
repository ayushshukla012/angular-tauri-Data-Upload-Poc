package com.insight.upload.repository;

import com.insight.upload.entity.Upload;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.UUID;

public interface UploadRepository extends JpaRepository<Upload, UUID> {
}
