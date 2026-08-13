package com.insight.transformation.repository;

import com.insight.transformation.entity.TransformationJob;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;
import java.util.UUID;

public interface TransformationJobRepository extends JpaRepository<TransformationJob, UUID> {

    Optional<TransformationJob> findByUploadId(UUID uploadId);
}
