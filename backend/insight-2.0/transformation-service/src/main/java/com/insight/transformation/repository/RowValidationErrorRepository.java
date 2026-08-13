package com.insight.transformation.repository;

import com.insight.transformation.entity.RowValidationError;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.UUID;

public interface RowValidationErrorRepository extends JpaRepository<RowValidationError, UUID> {
}
