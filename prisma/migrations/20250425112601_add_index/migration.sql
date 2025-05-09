/*
  Warnings:

  - The primary key for the `displays` table will be changed. If it partially fails, the table could be left without primary key constraint.

*/
-- AlterTable
ALTER TABLE "displays" DROP CONSTRAINT "displays_pkey",
ALTER COLUMN "id" SET DATA TYPE TEXT,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW(),
ADD CONSTRAINT "displays_pkey" PRIMARY KEY ("id");

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateIndex
CREATE INDEX "displays_object_type_idx" ON "displays"("object_type");
